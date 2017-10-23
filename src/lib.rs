#![recursion_limit="128"]

#[cfg_attr(test, macro_use)]
extern crate mail_codec_core;

#[macro_use]
extern crate log;
extern crate mime;
extern crate quoted_printable;
extern crate idna;
extern crate chrono;
extern crate futures;
extern crate serde;
extern crate base64;
extern crate rand;
extern crate percent_encoding;
extern crate soft_ascii_string;
extern crate total_order_multi_map;
extern crate tree_magic;

#[cfg_attr(test, macro_use)]
extern crate vec1;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate scoped_tls;


#[cfg(feature="default_impl_cpupool")]
extern crate futures_cpupool;


#[macro_use]
pub mod external;
#[cfg_attr(test, macro_use)]
pub mod codec;
pub mod data;
pub mod components;
#[macro_use]
pub mod headers;
pub mod mail;
pub mod composition;

#[cfg(feature="default_impl_any")]
pub mod default_impl;

pub mod mail_builder_prelude {
    pub type Encoder = ::codec::Encoder<::mail::Resource>;
    pub use error::*;
    pub use grammar::MailType;
    pub use codec::{EncodableInHeader, Encodable, EncodeHandle};
    pub use data::FromInput;
    pub use headers::*;
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
    pub type Encoder = ::codec::Encoder<::mail::Resource>;
    pub use error::*;
    pub use grammar::MailType;
    pub use data::FromInput;
    pub use components::{
        Mailbox,
        Email,
        TransferEncoding
    };
    pub use codec::{
        EncodableInHeader,
        EncodeHandle,
        Encodable
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

    pub use vec1::Vec1;
    pub use mail::{
        Resource
    };
    pub use composition::{
        Template, TemplateEngine,
        Context,
        Attachment, Embedding
    };
 }
