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

include!("includes/common.rs");

use rapt::*;
use serde::Serialize;

use std::thread;
use std::time::Duration;

#[derive(Clone, Serialize, Default, Debug)]
struct Datapoint {
    indicator: u32,
}

#[derive(Instruments)]
struct TestInstruments<L: Listener> {
    datapoint: Instrument<Datapoint, L>,
}

impl<L: Listener> Default for TestInstruments<L> {
    fn default() -> Self {
        TestInstruments{ datapoint: Instrument::default() }
    }
}

#[test]
#[cfg(feature = "timestamp_instruments")]
// Tests that instrument timestamp changes after an update, otherwise it doesn't
fn timestamp_changes() {
    let i = TestInstruments::<()>::default();

    let mut ser = serde_json::Serializer::new(Vec::with_capacity(128)) ;

    assert!(i.serialize_reading("datapoint", &mut ser).is_ok());
    let val1 = ser.into_inner();

    thread::sleep(Duration::from_millis(1));

    let mut ser = serde_json::Serializer::new(Vec::with_capacity(128)) ;
    assert!(i.serialize_reading("datapoint", &mut ser).is_ok());
    let val2 = ser.into_inner();
    assert_eq!(val1, val2);

    assert!(i.datapoint.update(|v| v.indicator = Default::default() ).is_ok());

    let mut ser = serde_json::Serializer::new(Vec::with_capacity(128)) ;
    assert!(i.serialize_reading("datapoint", &mut ser).is_ok());
    let val3 = ser.into_inner();

    assert_ne!(val1, val3);
}

#[test]
// Tests whether instruments work well in a multithreaded environment
fn multithread() {
    let i = TestInstruments::<()>::default();

    let i_ = i.datapoint.clone();
    let i__ = i.datapoint.clone();

    let t1 = thread::spawn(move || {
        for _ in 0..10000 {
            i_.update(|v| v.indicator += 1).unwrap();
        }
    });

    let t2 = thread::spawn(move || {
        for _ in 0..10000 {
            i__.update(|v| v.indicator += 1).unwrap();
        }
    });

    let _ = t1.join().unwrap();
    let _ = t2.join().unwrap();

    assert_eq!(i.datapoint.read().unwrap().indicator, 20000);
}

use std::sync::mpsc;

#[test]
// Tests wiring a listener
fn listener() {
    let (tx, rx) = mpsc::channel();

    let mut i = TestInstruments::default();
    i.wire_listener(tx);

    // We should have the first notification already (from the wiring)
    let res = rx.recv_timeout(Duration::from_millis(100));
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), "datapoint");

    // No more notifications at this time
    assert!(rx.try_recv().is_err());

    let _ = i.datapoint.update(|v| v.indicator = 100).unwrap();

    // We should have a new notification
    let res = rx.recv_timeout(Duration::from_millis(100));
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), "datapoint");

}