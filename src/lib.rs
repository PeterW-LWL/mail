#![recursion_limit="128"]

extern crate mail_codec as mail;
#[macro_use]
extern crate mail_codec_core as core;
extern crate mail_codec_headers as headers;
#[macro_use]
extern crate error_chain;
extern crate log;
extern crate mime;
extern crate futures;
extern crate serde;
extern crate rand;
extern crate soft_ascii_string;
extern crate total_order_multi_map;
extern crate chrono;
extern crate vec1;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate scoped_tls;


mod builder_extension;
pub use self::builder_extension::{
    BuilderExt
};

mod compositor;
pub use self::compositor::{
    Compositor, NameComposer,
};

mod utils;

mod context;
pub use self::context::{
    MailSendContext,
    ContentIdGen,
    Context,
    ComposedContext
};

mod resource;
pub use self::resource::{
    Embedding, EmbeddingWithCID,
    Attachment,
    BodyWithEmbeddings
};

mod template;
pub use self::template::{
    Template, TemplateEngine
};


#[cfg(feature="default_impl_any")]
pub mod default_impl;

pub mod resource_prelude {
    pub use mail::file_buffer::FileBuffer;
    pub use core::utils::FileMeta;
    pub use mail::{ Resource, ResourceSpec };
    pub use ::{ Embedding, Attachment, EmbeddingWithCID };
}

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
    pub use ::{
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
    pub use ::{
        Template, TemplateEngine,
        Context,
        Attachment, Embedding
    };
 }
