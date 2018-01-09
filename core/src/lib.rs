#![recursion_limit="128"]
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate nom;
extern crate owning_ref;
extern crate chrono;
extern crate mime;
extern crate total_order_multi_map;
extern crate soft_ascii_string;
extern crate base64;
extern crate quoted_printable;
extern crate idna;
extern crate quoted_string;
extern crate media_type_parser_utils;
extern crate percent_encoding;
extern crate vec1;
extern crate serde;
#[macro_use]
extern crate serde_derive;




//NOTE: this would be worth it's own independent crate for utility macros
#[macro_use]
pub mod macros;

#[macro_use]
pub mod utils;
pub mod error;
pub mod grammar;
#[cfg_attr(test, macro_use)]
pub mod codec;
pub mod data;
pub mod headers;
pub mod components;