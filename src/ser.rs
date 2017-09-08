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

//! # Serde Serializer utilities
//!
//! This module provides a mechanism for instantiating new
//! serializers over a given [`Write`]
//!
//! Some Rapt's components depend on the ability to be parametrized
//! over a serializer to use them interchangeably.
//!
//! Unfortunately, it is impossible to uniformly instantiate
//! different serializers. To ease this problem, this module
//! will add optional integrations with known serializers.
//!
//! Currently supported serializers are:
//!
//! * [`JsonSerializer`] — requires `serde_json` feature to be enabled; disabled by default
//!
//! The technique employed in this module depends on a common
//! pattern used in Serde ecosystem: actual serializers do not
//! implement [`Serializer`], but their mutable references (`&mut`) do.
//!
//! This allows to use the serializer (as its trait uses `self`) and be
//! able to retrieve the underlying writer.
//!
//! If some of the serializers does not employ the mutable reference
//! technique, perhaps, a wrapper can be implemented to fit it into the same
//! model.
//!
//! ## Example
//!
//! ```rust
//! extern crate rapt;
//! extern crate serde;
//!
//! use serde::{Serialize, Serializer};
//! use rapt::ser::{InstantiateSerializer, IntoWriter, JsonSerializer};
//! pub fn test<IS, S>(is: IS) -> Vec<u8>
//!     where for<'a> IS: InstantiateSerializer<'a, Vec<u8>, Target=S>,
//!     S: IntoWriter<Vec<u8>>, for<'a> &'a mut S: Serializer {
//!    let mut ser = is.instantiate_serializer(Vec::with_capacity(256));
//!    let _ = "test".serialize(&mut ser).unwrap();
//!    ser.into_writer()
//! }
//! fn main() {
//!   assert!(test(JsonSerializer).len() > 0);
//! }
//! ```
//!
//! [`Serializer`]: https://docs.serde.rs/serde/trait.Serializer.html
//! [`JsonSerializer`]: struct.JsonSerializer.html
//! [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
//!
use std::io::Write;

#[cfg(feature = "serde_json")]
use serde_json;

/// This trait instantiates a serializer over a given [`Write`]
///
/// Requires `Target` to be convertible back into the writer.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
pub trait InstantiateSerializer<'a, W: Write> {
    /// Target type
    ///
    /// This type *should not* implement [`Serializer`]
    ///
    /// [`Serializer`]: https://docs.serde.rs/serde/trait.Serializer.html
    type Target: IntoWriter<W>;
    /// Instantiate serializer over a writer
    fn instantiate_serializer(&self, over: W) -> Self::Target;
}

//// JSON Serializer (enabled in `serde_json` feature is enabled; disabled by default)
#[cfg(feature = "serde_json")]
pub struct JsonSerializer;

#[cfg(feature = "serde_json")]
impl<'a, W: Write + 'a> InstantiateSerializer<'a, W> for JsonSerializer {
    type Target = serde_json::Serializer<W>;

    fn instantiate_serializer(&self, over: W) -> Self::Target {
        serde_json::Serializer::new(over)
    }
}

/// Converts value into a writer
pub trait IntoWriter<W: Write> {
    /// Converts value into a writer
    fn into_writer(self) -> W;
}

#[cfg(feature = "serde_json")]
impl<W: Write> IntoWriter<W> for serde_json::Serializer<W> {
    fn into_writer(self) -> W {
        self.into_inner()
    }
}
