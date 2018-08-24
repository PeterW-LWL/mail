extern crate mail_types as mail;
extern crate mail_common as common;
#[macro_use]
extern crate mail_headers as headers;

#[macro_use]
extern crate failure;
extern crate mime as media_type;
extern crate futures;
extern crate soft_ascii_string;
#[macro_use]
extern crate vec1;

#[cfg(feature="serialize-to-content-id")]
extern crate serde;

#[cfg(feature="askama-engine")]
#[cfg_attr(test, macro_use)]
extern crate askama;

#[macro_use]
#[allow(unused_imports)]
extern crate mail_derive;

//re-export proc-macro
pub use mail_derive::*;

//modules are ordered in "after-can-import-from-before" order
pub mod error;
mod resource;
mod template_engine;
mod builder_extension;
mod compositor;

#[cfg(feature="askama-engine")]
pub mod askama_engine;

// re-exports flatten crate
pub use self::builder_extension::*;
pub use self::compositor::*;
pub use self::resource::*;
pub use self::template_engine::*;