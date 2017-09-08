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

//! # Rapt
//! #### Runtime application instrumentation toolkit
//!
//! Rapt provides a standard interface for providing runtime introspection capabilities
//! for Rust-based libraries and applications.
//!
//! There are a few key components to this library:
//!
//! ## Instrument
//!
//! [`Instrument`] is a thread-safe wrapper for a Serde-serializable value. It is parametrized
//! over [`Listener`]
//!
//! Instruments are cloneable and the wrapped value can be safely updated using [`Instrument#update`].
//!
//! ## Instrument board
//!
//! Instrument board is a concept of aggregating a number of [instruments](#instrument) into a
//! single structure and implementing or deriving [`Instruments`] for it. Please note that if
//! derivation is used (using `rapt_derive` crate), the last type parameter *must* be bound to
//! [`Listener`]:
//!
//! ```rust
//! extern crate rapt;
//! extern crate serde;
//! #[macro_use]
//! extern crate rapt_derive;
//!
//! use serde::Serialize;
//! use rapt::{Listener, Instrument};
//!
//! #[allow(dead_code)]
//! #[derive(Instruments)]
//! struct AppInstruments<T : Serialize, L: Listener> {
//!     value: Instrument<T, L>,
//! }
//!
//! pub fn main() {}
//! ```
//!
//! In the above example, `L` *must* always remain the last type parameter.
//!
//! It is parametrized over [`Listener`].
//!
//! ## Listener
//!
//! [`Listener`] is a trait that allows instruments to notify interested parties about updates
//!
//! ## Example
//!
//! ```rust
//! extern crate rapt;
//! extern crate serde;
//!
//! #[macro_use]
//! extern crate rapt_derive;
//! #[macro_use]
//! extern crate serde_derive;
//! #[macro_use]
//! extern crate assert_matches;
//!
//! use serde::Serialize;
//! use rapt::{Listener, Instrument};
//!
//! #[derive(Debug, Clone, Copy, Serialize)]
//! enum Status { Stopped, Started }
//!
//! #[derive(Clone, Serialize)]
//! struct Service {
//!     status: Status,
//! }
//!
//! #[derive(Instruments)]
//! struct AppInstruments<L: Listener> {
//!     http_server: Instrument<Service, L>,
//! }
//!
//! use std::thread;
//! use std::time::Duration;
//!
//! fn main() {
//!     let app_instruments = AppInstruments::<()> {
//!         http_server: Instrument::new(Service { status: Status::Stopped }),
//!     };
//!     let http_server_svc = app_instruments.http_server.clone();
//!     let thread_handle = thread::spawn(move || {
//!         thread::sleep(Duration::from_millis(100));
//!         let _ = http_server_svc.update(|v| v.status = Status::Started).unwrap();
//!     });
//!     thread::sleep(Duration::from_millis(200));
//!     assert_matches!(app_instruments.http_server.read().and_then(|v| Ok(v.status)), Ok(Status::Started));
//!     let _ = thread_handle.join().unwrap();
//! }
//! ```
//!
//! [`Instrument`]: struct.Instrument.html
//! [`Instrument#update`]: struct.Instrument.html#method.update
//! [`Instruments`]: struct.Instruments.html
//! [`Listener`]: trait.Listener.html

extern crate serde;

use serde::{Serialize, Serializer};
use serde::ser::SerializeStruct;

use std::sync::{Arc, RwLock, RwLockReadGuard, LockResult};

#[cfg(feature = "timestamp_instruments")]
extern crate chrono;
#[cfg(feature = "timestamp_instruments")]
use chrono::prelude::*;

/// A thread-safe wrapper for a Serde-serializable value
///
/// It is parametrized over [`Listener`]
///
/// Instruments are cloneable and the wrapped value can be safely updated using [`Instrument#update`].
///
/// [`Listener`]: trait.Listener.html
#[derive(Clone)]
pub struct Instrument<T: Serialize, L: Listener> {
    data: Arc<RwLock<T>>,
    name: Option<&'static str>,
    listener: Option<L>,
    #[cfg(feature = "timestamp_instruments")]
    timestamp: Arc<RwLock<DateTime<Utc>>>,
}

/// An error that might occur during [`Instrument#update`]
///
/// [`Instrument#update`]: struct.Instrument.html#method.update
#[derive(Debug)]
pub enum UpdateError {
    PoisonedData,
    PoisonedTimestamp,
}

impl<T: Serialize + Default, L: Listener> Default for Instrument<T, L> {
    fn default() -> Self {
        Instrument {
            data: Default::default(),
            name: None,
            listener: None,
            #[cfg(feature = "timestamp_instruments")]
            timestamp: Arc::new(RwLock::new(Utc::now())),
        }
    }
}

impl<T: Serialize, L: Listener> Instrument<T, L> {
    /// Creates a new instrument
    pub fn new(data: T) -> Self {
        Instrument {
            data: Arc::new(RwLock::new(data)),
            name: None,
            listener: None,
            #[cfg(feature = "timestamp_instruments")]
            timestamp: Arc::new(RwLock::new(Utc::now())),
        }
    }

    fn serialization_field_count() -> usize {
        #[allow(unused_mut)]
        let mut c = 1;
        if cfg!(feature = "timestamp_instruments") {
            c += 1;
        }
        c
    }

    /// Sets the name of the instrument. FOR INTERNAL USE ONLY.
    pub fn set_name(&mut self, name: &'static str) {
        self.name = Some(name)
    }

    /// Sets the name of the instrument and the listener. FOR INTERNAL USE ONLY.
    pub fn set_name_and_listener(&mut self, name: &'static str, listener: L) {
        self.name = Some(name);
        listener.instrument_updated(name);
        self.listener = Some(listener);
    }

    /// Thread-safe value reader
    pub fn read(&self) -> LockResult<RwLockReadGuard<T>> {
        self.data.read()
    }

    /// Thread-safe value writer
    pub fn update<F>(&self, f: F) -> Result<(), UpdateError> where F: Fn(&mut T) -> () {
        match self.data.write() {
            Ok(mut data) => {
                f(&mut *data);
                match self.timestamp.write() {
                    Ok(mut timestamp) => {
                        *timestamp = Utc::now();
                        match (&self.listener, &self.name) {
                            (&Some(ref l), &Some(ref n)) => l.instrument_updated(n),
                            _ => (),
                        }
                        Ok(())
                    },
                    Err(_) => Err(UpdateError::PoisonedData),
                }
            },
            Err(_) => Err(UpdateError::PoisonedData),
        }
    }
}
impl<T: Serialize, L: Listener> Serialize for Instrument<T, L> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer {
        let mut ss = serializer.serialize_struct("Instrument", Instrument::<T, L>::serialization_field_count())?;
        match self.data.read() {
            Ok(res) => ss.serialize_field("value", &Some(&*res))?,
            Err(_) => ss.serialize_field("value", &None::<T>)?,
        }
        if cfg!(feature = "timestamp_instruments") {
            ss.serialize_field("last_update_at", &&*self.timestamp)?;
        }
        ss.end()
    }
}


/// An error that might occur during [`Instrument#read`]
///
/// [`Instrument#read`]: struct.Instrument.html#method.read
#[derive(Debug)]
pub enum ReadError<E> {
    SerializationError(E),
    NotFound
}

/// Instrument board trait
///
/// Instrument board is a concept of aggregating a number of instruments into a
/// single structure and implementing or deriving `Instruments` for it. Please note that if
/// derivation is used (using `rapt_derive` crate), the last type parameter *must* be bound to
/// [`Listener`]
/// [`Listener`]: trait.Listener.html
pub trait Instruments<L: Listener> {
    /// Serialize a particular instrument if it is present, fail otherwise.
    fn serialize_reading<K : AsRef<str>, S: Serializer>(&self, key: K, serializer: S) -> Result<S::Ok, ReadError<S::Error>>;
    /// Returns a list of instrument names
    fn instrument_names(&self) -> Vec<&'static str>;
    /// Wires listener into all instruments. If not used, no update notifications will be delivered
    fn wire_listener(&mut self, listener: L);
}

/// Trait that allows instruments to notify interested parties about updates
pub trait Listener : Clone {
    /// When invoked, an instrument with a `name` has been updated.
    fn instrument_updated(&self, name: &'static str);
}

/// `()` implements [`Listener`] and silently discards updates. It essentially means
/// "no listener"
/// [`Listener`]: trait.Listener.html
impl Listener for () {
    #[allow(unused_variables)]
    fn instrument_updated(&self, name: &'static str) {}
}

use std::sync::mpsc;

/// `mpsc::Sender<&'static str>` implements [`Listener`] and delivers the notifications
/// over that channel.
/// [`Listener`]: trait.Listener.html
impl Listener for mpsc::Sender<&'static str> {
    #[allow(unused_variables)]
    fn instrument_updated(&self, name: &'static str) {
        let _ = self.send(name).unwrap();
    }
}

/// Declare and re-export optional mqttc crate
#[cfg(feature = "mqtt_publisher")]
pub extern crate mqttc;
/// Optional mqtt module
#[cfg(feature = "mqtt_publisher")]
pub mod mqtt;

/// Declare and re-export optional serde_json crate
#[cfg(feature = "serde_json")]
pub extern crate serde_json;

/// Serialization utilities
pub mod ser;