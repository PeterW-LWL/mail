#![recursion_limit="128"]
#![cfg_attr(feature="specialization", feature(specialization))]
extern crate mail_types as mail;
extern crate mail_common as common;
#[macro_use]
extern crate mail_headers as headers;

#[macro_use]
extern crate failure;
extern crate log;
extern crate mime as media_type;
extern crate futures;
extern crate rand;
extern crate soft_ascii_string;
extern crate chrono;
#[macro_use]
extern crate vec1;
#[cfg(feature="render-template-engine")]
extern crate conduit_mime_types;
#[cfg(feature="render-template-engine")]
#[macro_use]
extern crate lazy_static;
#[cfg(feature="tera-bindings")]
extern crate tera as tera_crate;
#[macro_use]
#[allow(unused_imports)]
extern crate mail_derive;

//re-export proc-macro
pub use mail_derive::*;

//modules are ordered in "after-can-import-from-before" order
#[macro_use]
mod macros;
pub mod error;
mod resource;
mod template_engine;
mod builder_extension;
mod compositor;

#[cfg(feature="render-template-engine")]
pub mod render_template_engine;
#[cfg(feature="tera-bindings")]
pub mod tera;

//TODO consider using glob reexports and pub(crate) for
// non public parts used by other modules

// reexports
pub use self::builder_extension::BuilderExt;
pub use self::compositor::*;
pub use self::resource::*;
pub use self::template_engine::*;