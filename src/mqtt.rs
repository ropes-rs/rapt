// Copyright 2017 All Contributors (see CONTRIBUTORS file)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! # MQTT Publisher
//!
//! _This module is only present if `mqtt_publisher` feature is enabled.
//! It is disabled by default._
//!
//! [MQTT] publisher is one of the ways to publish application's instruments
//! to external consumers. Unlike a traditional approach of having a server
//! embedded into the application and have a protocol for reading values,
//! streaming updates, and so on, this approach does not embed any server.
//!
//! Instead, it connects over to an MQTT broker and sends the data to it,
//! as well as receives some control messages from it as well.
//!
//! This approach has numerous advantages:
//!
//! * It makes it possible to monitor and interact with multiple applications through
//!   one connection, as opposed to connecting to all of them (which requires
//!   maintaining a service resolution system in place as well)
//! * By employing message retaining flag, the "last known" data is always stored with
//!   the broker, even if the application is down, the consumer can still read the value
//!   (if this behaviour is appropriate for the problem)
//! * It makes firewall configuration simpler (no need to open a port for this service on
//!   each node) and more secure (no ingress)
//!
//! ## Example
//!
//! You can find an example on how to use MQTT publisher in Rapt's crate repository, in
//! the examples directory (`exampels/mqtt.rs`)
//!
//! [MQTT]: http://mqtt.org/

/// Re-exports mqtcc crate
pub use mqttc as client;
use self::client::{PubSub, PubOpt};

use super::{Listener, Instruments};
use super::ser::{InstantiateSerializer, IntoWriter};
use serde::Serializer;

use std::sync::mpsc;

/// Publisher control messages
enum Message {
    /// An instrument has been updated
    Update(&'static str),
    /// Shutdown requested
    Shutdown,
}

/// A trait for formatting instrument name into a full MQTT topic name
pub trait TopicFormatter {
    fn format_topic(&self, name: &'static str) -> String;
}

/// `()` as a [`TopicFormatter`] simply returns instrument name as a topic
///
/// [`TopicFormatter`]: trait.TopicFormatter.html
impl TopicFormatter for () {
    fn format_topic(&self, name: &'static str) -> String {
        name.into()
    }
}

/// MQTT publisher
/// 
/// An important aspect of how Rapt and `Publisher` works is that it *will not*
/// publish all updates, especially if they are being updated fast. It *will* react
/// to every event of an update but it will grab instrument's last value as opposed
/// to the value that it had after that particular update. As a consequence, `Publisher`
/// will filter out messages that simply repeat the previous message for the given instrument.
pub struct Publisher<TF: TopicFormatter, I: Instruments<Handle>> {
    topic_formatter: TF,
    client: client::Client,
    instruments: I,
    retain: bool,
    sender: mpsc::Sender<Message>,
    receiver: mpsc::Receiver<Message>,
}

impl<TF: TopicFormatter, I: Instruments<Handle>> Publisher<TF, I> {
    /// Creates a new MQTT publisher
    ///
    /// Consumes following arguments:
    ///
    /// * a topic formatter
    /// * a *connected* client
    /// * instruments
    /// * retain (true if messages should be retained)
    ///
    pub fn new(topic_formatter: TF, client: client::Client, mut instruments: I, retain: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let handle = Handle { sender: sender.clone() };
        instruments.wire_listener(handle);
        Publisher {
            topic_formatter,
            client,
            instruments,
            retain,
            sender,
            receiver,
        }
    }

    /// Returns a reference to instruments
    ///
    /// This is an important method as it allows to access instruments after the instrument board
    /// has been consumed by `Publisher`:
    ///
    /// ```norun
    /// let mut publisher = mqtt::Publisher::new((), client, instruments, true);
    /// let datapoint = publisher.instruments().main_value.clone();
    /// ```
    pub fn instruments(&self) -> &I {
        &self.instruments
    }

    /// Handle to the running `Publisher`
    ///
    /// Mainly used to gracefully shut it down.
    pub fn handle(&self) -> Handle {
        Handle { sender: self.sender.clone() }
    }

    /// This method is typically used to run the publisher in a new thread:
    ///
    /// ```norun
    /// let publisher_thread = thread::spawn(move || publisher.run(rapt::ser::JsonSerializer));
    /// ```
    pub fn run<IS, S>(&mut self, is: IS)
           where for<'a> IS: InstantiateSerializer<'a, Vec<u8>, Target=S>,
                 S: IntoWriter<Vec<u8>>, for<'a> &'a mut S: Serializer {

        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        use std::collections::HashMap;
        use std::collections::hash_map::Entry;
        // This allows us to filter out duplicate values, by storing
        // `name => serialized_value_hash` we can relatively quickly
        // and inexpensively check whether we're attempting to send
        // a duplicate of the last message
        let mut last_messages = HashMap::new();

        let pubopt = if self.retain {
            PubOpt::retain()
        } else {
            PubOpt::at_least_once()
        };
        loop {
            match self.receiver.recv() {
                Ok(Message::Shutdown) => break,
                Ok(Message::Update(name)) => {
                    let mut ser = is.instantiate_serializer(Vec::with_capacity(64));
                    let _ = self.instruments.serialize_reading(name, &mut ser).unwrap();
                    let vec : Vec<u8> = ser.into_writer();

                    // Calculate message hash
                    let mut hasher = DefaultHasher::new();
                    vec.hash(&mut hasher);
                    let hash = hasher.finish();

                    if match last_messages.entry(name) {
                        // This is the first message for this instrument
                        Entry::Vacant(entry) => {
                            entry.insert(hash);
                            // send it
                            true
                        },
                        // There was a message sent for this instrument
                        Entry::Occupied(mut entry) => {
                            if *entry.get() != hash {
                                entry.insert(hash);
                                // if it was a different message, send it
                                true
                            } else {
                                // otherwise, don't
                                false
                            }
                        }
                    } {
                        let _ = self.client.publish(self.topic_formatter.format_topic(name), vec, pubopt).unwrap();
                    }
                },
                Err(err) => panic!(err),
            }
        }
    }

    /// Consumes `Publisher` and returns underlying `Client`
    pub fn into_inner(self) -> client::Client {
        self.client
    }
}

/// Running [`Publisher`] handle
///
/// [`Publisher`]: struct.Publisher.html
#[derive(Clone)]
pub struct Handle {
    sender: mpsc::Sender<Message>,
}

impl Handle {
    /// Shutdown the publisher
    pub fn shutdown(&self) {
        let _ = self.sender.send(Message::Shutdown).unwrap();
    }
}

/// Very importantly, [`Handle`] is a [`Listener`],
///
/// [`Handle`]: struct.Handle.html
/// [`Listener`]: ../trait.Listener.html
impl Listener for Handle {
    fn instrument_updated(&self, name: &'static str) {
        let _ = self.sender.send(Message::Update(name)).unwrap();
    }
}