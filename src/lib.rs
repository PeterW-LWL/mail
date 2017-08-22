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

#[macro_use]
pub mod utils;
pub mod error;
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

pub mod mail_builder_prelude {
    pub use error::*;
    pub use grammar::MailType;
    pub use codec::{
        MailEncodable,
        MailEncoderImpl
    };
    pub use data::FromInput;
    pub use headers::Header;
    pub use components::*;
    pub use mail::Builder;
    pub use mail::mime::MultipartMime;
}


pub mod resource_prelude {
    pub use utils::FileBuffer;
    pub use utils::FileMeta;
    pub use mail::{ Resource, ResourceSpec };
    pub use composition::{ Embedding, Attachment, EmbeddingWithCID };
}

pub mod composition_prelude {
    pub use error::*;
    pub use grammar::MailType;
    pub use data::FromInput;
    pub use components::{
        Mailbox,
        Email,
        TransferEncoding
    };
    pub use codec::{
        MailEncodable,
        MailEncoderImpl
    };
    pub use composition::{
        Compositor,
        NameComposer,
        MailSendContext,
    };
}

pub mod template_engine_prelude {
    pub type StdError = ::std::error::Error;
    pub type StdResult<R,E> = ::std::result::Result<R,E>;
    pub use serde::Serialize;

    pub use utils::Vec1;
    pub use mail::{
        Resource
    };
    pub use composition::{
        Template, TemplateEngine,
        Context,
        Attachment, Embedding
    };
 }
