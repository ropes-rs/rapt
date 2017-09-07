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

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate rapt;

#[macro_use]
extern crate rapt_derive;

extern crate rmp_serde as serde_msgpack;

#[macro_use]
extern crate assert_matches;

use rapt::*;
use serde::Serialize;


#[derive(Serialize, Default)]
struct Datapoint {
    value: u32,
}

#[derive(Instruments, Default)]
struct TestInstruments<L: Listener> {
    dp: Instrument<Datapoint, L>,
    #[rapt(name = "info")]
    dp1: Instrument<Datapoint, L>,
}


#[test]
fn reading_field_name() {
    let i = TestInstruments::<()>::default();

    let mut ser = serde_msgpack::Serializer::new_named(Vec::with_capacity(128)) ;
    let res = i.serialize_reading("dp", &mut ser);
    assert!(res.is_ok());
    let v = ser.into_inner();
    assert!(!v.is_empty());
}

#[test]
fn missing_name() {
    let i = TestInstruments::<()>::default();

    let mut ser = serde_msgpack::Serializer::new_named(Vec::with_capacity(128)) ;
    let res = i.serialize_reading("missing_name", &mut ser);
    assert!(res.is_err());
    assert_matches!(res.unwrap_err(), ReadError::NotFound);
}

#[test]
fn name_attribute() {
    let i = TestInstruments::<()>::default();

    let mut ser = serde_msgpack::Serializer::new_named(Vec::with_capacity(128)) ;
    let res = i.serialize_reading("info", &mut ser);
    assert!(res.is_ok());
    let v = ser.into_inner();
    assert!(!v.is_empty());
}

#[test]
fn names() {
    let i = TestInstruments::<()>::default();

    assert_eq!(vec!["dp", "info"], i.instrument_names());
}