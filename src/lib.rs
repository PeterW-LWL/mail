#![recursion_limit="128"]
//TODO remove that
extern crate ascii;
extern crate mime;
extern crate owning_ref;
extern crate quoted_printable;
extern crate chrono;
extern crate futures;
extern crate serde;
extern crate base64;
extern crate rand;
//#[macro_use]
//extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate scoped_tls;

#[macro_use]
extern crate nom;

#[macro_use]
extern crate error_chain;


#[cfg(feature="default_impl_cpupool")]
extern crate futures_cpupool;

#[macro_use]
mod macros;

mod utils;
pub mod error;
#[macro_use]
pub mod types;
pub mod grammar;
#[cfg_attr(test, macro_use)]
pub mod codec;
pub mod data;
pub mod components;
pub mod headers;
pub mod mail;
pub mod composition;

#[cfg(feature="default_impl_any")]
pub mod default_impl;




