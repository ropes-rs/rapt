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

use std::env;
use std::process::exit;
use std::time::Duration;
use std::thread;


extern crate serde;
#[macro_use]
extern crate serde_derive;

use serde::Serialize;

extern crate rapt;
#[macro_use]
extern crate rapt_derive;

extern crate netopt;

use netopt::NetworkOptions;
use rapt::mqtt::client::{ClientOptions, ReconnectMethod};
use rapt::{mqtt, Instrument, Listener};

#[derive(Clone, Serialize, Default, Debug)]
struct Datapoint {
    indicator: u32,
}

#[derive(Instruments)]
struct TestInstruments<L: Listener> {
    #[rapt(name = "value/main")]
    main_value: Instrument<Datapoint, L>,
    #[rapt(name = "value/supplemental")]
    supplemental_value: Instrument<Datapoint, L>,
}

impl<L: Listener> Default for TestInstruments<L> {
    fn default() -> Self {
        TestInstruments{ main_value: Instrument::default(), supplemental_value: Instrument::default() }
    }
}

pub fn main() {

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run --example mqtt --features mqtt_publisher,netopt,serde_json -- 127.0.0.1:1883");
        exit(1);
    }

    let ref address = args[1];


    let netopt = NetworkOptions::new();
    let mut opts = ClientOptions::new();
    opts.set_keep_alive(15);
    opts.set_reconnect(ReconnectMethod::ReconnectAfter(Duration::new(5,0)));
    let client = opts.connect(address.as_str(), netopt).unwrap();

    let instruments = TestInstruments::default();
    let mut publisher = mqtt::Publisher::new((), client, instruments, true);

    let datapoint = publisher.instruments().main_value.clone();

    let publisher_handle = publisher.handle();
    let publisher_thread = thread::spawn(move || publisher.run(rapt::ser::JsonSerializer));

    let service_thread = thread::spawn(move ||
       for _ in 0..100 {
           let _ = datapoint.update(|v| v.indicator += 1).unwrap();
       }
    );

    let _ = service_thread.join().unwrap();
    publisher_handle.shutdown();
    let _ = publisher_thread.join().unwrap();

}