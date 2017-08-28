//FIXME use FnvHashMap
use std::collections::HashMap;
use std::collections::hash_map::{ Iter as MapIter };
use std::borrow::Cow;

use codec::{ MailEncoder, MailEncodable };
use ascii::{ AsciiString, AsciiStr, AsciiChar };
use futures::{ Future, Async, Poll };

use error::*;
use utils::is_multipart_mime;
use headers::Header;

pub use self::builder::*;
pub use self::context::*;
pub use self::resource::*;

pub mod mime;
mod resource;
mod builder;
mod encode;
mod context;


type Headers = HashMap<Cow<'static, AsciiStr>, Header>;
type HeadersIter<'a> = MapIter<'a, Cow<'static, AsciiStr>, Header>;

pub struct Mail {
    //NOTE: by using some OwnedOrStaticRef AsciiStr we can probably safe a lot of
    // unnecessary allocations
    headers: Headers,
    body: MailPart,
}


pub enum MailPart {
    SingleBody {
        body: Resource
    },
    MultipleBodies {
        bodies: Vec<Mail>,
        hidden_text: AsciiString
    }
}

/// a future returning an EncodableMail once all futures contained in the wrapped Mail are resolved
pub struct MailFuture<'a, T: 'a> {
    mail: Option<Mail>,
    ctx: &'a T
}

/// a mail with all contained futures resolved, so that it can be encoded
pub struct EncodableMail( Mail );

impl Mail {


    /// adds a new header,
    ///
    /// - if the header already existed, the existing one will be overriden and the
    ///   old header will be returned
    /// - `Content-Transfer-Encoding` it might be overwritten later one
    ///
    /// # Failure
    ///
    /// if a Content-Type header is set, which conflicts with the body, mainly if
    /// you set a multipart content type on a non-multipart body or the other way around
    ///
    pub fn set_header( &mut self, header: Header ) -> Result<Option<Header>> {
        use headers::Header::*;

        match &header {
            &ContentType( ref mime ) => {
                if self.body.is_multipart() != is_multipart_mime( mime ) {
                    return Err( ErrorKind::ContentTypeAndBodyIncompatible.into() )
                }
            },
            &ContentTransferEncoding( ref _encoding ) => {
                //TODO warn as this is most likly leading to unexpected results
            },
            _ => {}
        }

        Ok( self.headers.insert( header.name().into(), header ) )

    }

    pub fn headers<'a>( &'a self ) -> HeadersIter<'a> {
        self.headers.iter()
    }

    pub fn body( &self ) -> &MailPart {
        &self.body
    }

    pub fn into_future<'a, C: BuilderContext>( self, ctx: &'a C ) -> MailFuture<'a, C> {
        MailFuture {
            ctx,
            mail: Some( self )
        }
    }

    fn walk_mail_bodies_mut<FN>( &mut self, use_it_fn: &mut FN) -> Result<()>
        where FN: FnMut( &mut Resource ) -> Result<()>
    {
        use self::MailPart::*;
        match self.body {
            SingleBody { ref mut body } =>
                use_it_fn( body )?,
            MultipleBodies { ref mut bodies, .. } =>
                for body in bodies {
                    body.walk_mail_bodies_mut( use_it_fn )?
                }
        }
        Ok( () )
    }
}







impl MailPart {

    pub fn is_multipart( &self ) -> bool {
        use self::MailPart::*;
        match *self {
            SingleBody { .. } => false,
            MultipleBodies { .. } => true
        }
    }
}


impl<'a, T> Future for MailFuture<'a, T> where T: BuilderContext {
    type Item = EncodableMail;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut done = true;
        let ctx: &T = &self.ctx;
        self.mail.as_mut()
            // this is conform with how futures work, as calling poll on a random future
            // after it completes has unpredictable results (through one of NotRady/Err/Panic)
            // use `Fuse` if you want more preditable behaviour in this edge case
            .expect( "poll not to be called after completion" )
            .walk_mail_bodies_mut( &mut |body: &mut Resource| {
                match body.poll_encoding_completion( ctx ) {
                    Ok( Async::NotReady ) => {
                        done = false;
                        Ok(())
                    },
                    Ok( Async::Ready( .. ) ) => {
                        Ok(())
                    },
                    Err( err ) => {
                         Err( err )
                    }
                }
            })?;

        if done {
            Ok( Async::Ready( EncodableMail( self.mail.take().unwrap() ) ) )
        } else {
            Ok( Async::NotReady )
        }
    }
}


deref0!{ -mut EncodableMail => Mail }

impl Into<Mail> for EncodableMail {
    fn into( self ) -> Mail {
        self.0
    }
}

impl<E> MailEncodable<E> for EncodableMail where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        // does not panic as a EncodableMail only is constructed from
        // a Mail which has all of it's bodies resolved, without failure
        encode::encode_mail( &self, true, encoder )
    }
}