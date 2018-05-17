#![recursion_limit="128"]

extern crate mail_types as mail;
extern crate mail_common as common;
#[macro_use]
extern crate mail_headers as headers;

#[macro_use]
extern crate failure;
extern crate log;
extern crate mime as media_type;
extern crate futures;
extern crate serde;
extern crate rand;
extern crate soft_ascii_string;
extern crate chrono;
#[macro_use]
extern crate vec1;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate scoped_tls;
#[cfg(feature="default_impl_simple_context")]
extern crate futures_cpupool;
#[cfg(feature="render-template-engine")]
extern crate conduit_mime_types;
#[cfg(feature="render-template-engine")]
#[macro_use]
extern crate lazy_static;
#[cfg(feature="tera-bindings")]
extern crate tera as tera_crate;

//modules are ordered in "after-can-import-from-before" order
#[macro_use]
mod macros;
mod utils;
pub mod error;
mod context;
mod resource;
mod template;
mod builder_extension;
mod compositor;
pub mod default_impl;

#[cfg(feature="render-template-engine")]
pub mod render_template_engine;
#[cfg(feature="tera-bindings")]
pub mod tera;

//TODO consider using glob reexports and pub(crate) for
// non public parts used by other modules

// reexports
pub use self::builder_extension::BuilderExt;
pub use self::compositor::{
    CompositionBase, NameComposer,
    MailSendData, MailSendDataBuilder,
    SharedCompositionBase, SimpleCompositionBase
};
pub use self::context::{
    Context,
    ContentIdGenComponent,
    CompositeContext
};
pub use self::resource::{
    Embedding, EmbeddingWithCId,
    Attachment,
};
pub use self::template::{
    MailParts, BodyPart, TemplateEngine
};
//################# preludes ###########################

// pub mod resource_prelude {
//     pub use mail::file_buffer::FileBuffer;
//     pub use common::utils::FileMeta;
//     pub use mail::Resource;
//     pub use mail::context::Source;
//     pub use ::{Embedding, Attachment, EmbeddingWithCId};
// }

// pub mod composition_prelude {
//     pub type Encoder = ::common::codec::Encoder<::mail::Resource>;
//     pub use common::*;
//     pub use headers::components::{
//         Mailbox,
//         Email,
//         TransferEncoding,
//         MediaType
//     };
//     pub use common::codec::{
//         EncodableInHeader,
//         EncodeHandle,
//         Encodable
//     };
//     pub use ::{
//         CompositionBase,
//         SimpleCompositionBase,
//         SharedCompositionBase,
//         NameComposer,
//         MailSendData,
//     };
// }

// pub mod template_engine_prelude {
//     pub use serde::Serialize;

//     pub use vec1::Vec1;
//     pub use mail::mail::{
//         Resource
//     };
//     pub use mail::context::Source;
//     pub use ::{
//         MailParts, BodyPart, TemplateEngine,
//         Context,
//         Attachment, EmbeddingWithCId
//     };
//  }
