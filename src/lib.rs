#![recursion_limit="128"]

extern crate mail_codec as mail;
#[macro_use]
extern crate mail_codec_core as core;
extern crate mail_codec_headers as headers;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;
extern crate mime;
extern crate futures;
extern crate serde;
extern crate rand;
extern crate soft_ascii_string;
extern crate total_order_multi_map;
extern crate chrono;


#[cfg_attr(test, macro_use)]
extern crate vec1;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate scoped_tls;


#[cfg(feature="default_impl_cpupool")]
extern crate futures_cpupool;

#[cfg(feature="default_impl_any")]
pub mod default_impl;

pub mod composition_prelude {
    pub type Encoder = ::core::codec::Encoder<::mail::Resource>;
    pub use core::*;
    pub use core::error::*;
    pub use core::grammar::MailType;
    pub use headers::components::{
        Mailbox,
        Email,
        TransferEncoding,
        MediaType
    };
    pub use core::codec::{
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
    pub use mail::mail::{
        Resource
    };
    pub use composition::{
        Template, TemplateEngine,
        Context,
        Attachment, Embedding
    };
 }
