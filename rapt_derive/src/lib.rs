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
#![recursion_limit = "128"]

extern crate syn;
use syn::{Ident, Body, MetaItem, NestedMetaItem, Lit};
use quote::Tokens;

#[macro_use]
extern crate quote;

extern crate proc_macro;
use proc_macro::TokenStream;

#[derive(Clone)]
struct InstrumentField { name: String, ident: Ident }
#[proc_macro_derive(Instruments, attributes(rapt))]
pub fn derive_instruments(input: TokenStream) -> TokenStream {
    let input = syn::parse_derive_input(&input.to_string()).unwrap();
    let ident = input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let listener_ident = &input.generics.ty_params.iter().last().unwrap().ident;
    let dummy_const = Ident::new(format!("_IMPL_INSTRUMENTS_FOR_{}", ident));

    match input.body {
        Body::Enum(_) => panic!("enums are not supported for Instruments derivations"),
        Body::Struct(variants) => {
            let instruments : Vec<InstrumentField> = variants.fields().iter().enumerate()
                .map(|(i, f)| {
                    let overriding_name = match f.attrs.iter()
                        .find(|a| a.name() == "rapt") {
                           Some(attr) => match attr.value {
                               MetaItem::List(_, ref items) =>
                                   items.iter().find(|item| match item {
                                       &&NestedMetaItem::MetaItem(ref item) => item.name() == "name",
                                       _ => false,
                                   }).map(|item| match item {
                                        &NestedMetaItem::MetaItem(MetaItem::NameValue(_, Lit::Str(ref str, _))) => str.clone(),
                                       _ =>
                                           panic!("#[rapt(name = \"...\") attribute can only contain a string value"),
                                   }),
                               _ => None,
                           },
                           None => None,
                    };
                    if f.ident.is_none() && overriding_name.is_none() {
                        panic!("struct {:} can't derive Instruments because field #{:} has no #[rapt(name = \"..\")] attribute", ident, i);
                    }
                    let name = if overriding_name.is_some() {
                        overriding_name.unwrap()
                    } else {
                        String::from(f.ident.clone().unwrap().as_ref())
                    };
                    InstrumentField { name, ident: f.ident.clone().unwrap() }
            }).collect();
            let matches : Vec<Tokens> = instruments.clone().into_iter().map(|i| {
                    let (name, ident) = (i.name, i.ident);
                    quote!{ #name => self . #ident . serialize(serializer).map_err(|e| _rapt::ReadError::SerializationError(e))  }
                }).collect();
            let names : Vec<Tokens> = instruments.clone().into_iter().map(|i| {
                let name = i.name;
                quote!{ #name }
            }).collect();
            let wirings : Vec<Tokens> = instruments.clone().into_iter().map(|i| {
                let (name, ident) = (i.name, i.ident);
                quote!{
                    self . #ident . set_name_and_listener(#name, listener.clone())
                }
            }).collect();
            let impl_block = quote! {
                impl #impl_generics _rapt::Instruments<#listener_ident> for #ident #ty_generics #where_clause {
                   fn serialize_reading<K : AsRef<str>, S: _serde::Serializer>(&self, key: K, serializer: S) -> Result<S::Ok, _rapt::ReadError<S::Error>> {
                      match key.as_ref() {
                        #(#matches),*,
                           _ => Err(_rapt::ReadError::NotFound),
                      }
                   }
                   fn instrument_names(&self) -> Vec<&'static str> {
                      vec![#(#names),*]
                   }
                   fn wire_listener(&mut self, listener: L) {
                      #(#wirings);*
                   }
                }
            };

            let generated = quote! {
                #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
                const #dummy_const: () = {
                    extern crate rapt as _rapt;
                    extern crate serde as _serde;
                    #impl_block
                };
            };
            generated.parse().unwrap()
        },
    }
}
