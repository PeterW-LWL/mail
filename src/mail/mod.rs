use std::ops::Deref;
use std::fmt;

use codec::{ MailEncoder, MailEncodable };
use ascii::{ AsciiString, AsciiChar };
use futures::{ Future, Async, Poll };

use error::*;
use utils::HeaderTryInto;
use headers::{
    Header, HeaderMap,
    ContentType, From,
    ContentTransferEncoding,
    Date, MessageId
};
use components::DateTime;

use self::builder::{
    check_header,
    check_multiple_headers,
};

pub use self::builder::{
    Builder, MultipartBuilder, SinglepartBuilder
};
pub use self::context::*;
pub use self::resource::*;

pub mod mime;
mod resource;
mod builder;
mod encode;
mod context;



pub struct Mail<E: MailEncoder> {
    //NOTE: by using some OwnedOrStaticRef AsciiStr we can probably safe a lot of
    // unnecessary allocations
    headers: HeaderMap<E>,
    body: MailPart<E>,
}


pub enum MailPart<E: MailEncoder> {
    SingleBody {
        body: Resource
    },
    MultipleBodies {
        bodies: Vec<Mail<E>>,
        hidden_text: AsciiString
    }
}

/// a future returning an EncodableMail once all futures contained in the wrapped Mail are resolved
pub struct MailFuture<'a, T: 'a, E: MailEncoder> {
    mail: Option<Mail<E>>,
    ctx: &'a T
}

/// a mail with all contained futures resolved, so that it can be encoded
pub struct EncodableMail<E: MailEncoder>( Mail<E> );

impl<E> Mail<E>
    where E: MailEncoder
{


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
    pub fn set_header<H, C>( &mut self, header: H, comp: C) -> Result<()>
        where H: Header,
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        let comp = comp.try_into()?;
        check_header::<H, _>( &comp, self.body.is_multipart() )?;
        self.headers.insert( header, comp )?;
        Ok( () )
    }

    pub fn set_headers( &mut self, headers: HeaderMap<E> ) -> Result<()> {
        check_multiple_headers( &headers, self.body.is_multipart() )?;
        if let Err(errs) = self.headers.extend( headers ) {
            let errors = errs.into_iter().map(|(hn,_,_,err)| {
                err.chain_err(||ErrorKind::FailedToAddHeader(hn.as_str()))
            }).collect::<Vec<_>>();
            bail!(ErrorKind::MultipleErrors(errors.into()));
        }
        Ok( () )
    }

    pub fn headers( &self ) -> &HeaderMap<E> {
        &self.headers
    }

    pub fn body( &self ) -> &MailPart<E> {
        &self.body
    }

    pub fn into_future<'a, C: BuilderContext>( self, ctx: &'a C ) -> MailFuture<'a, C, E> {
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







impl<E> MailPart<E>
    where E: MailEncoder
{

    pub fn is_multipart( &self ) -> bool {
        use self::MailPart::*;
        match *self {
            SingleBody { .. } => false,
            MultipleBodies { .. } => true
        }
    }
}


impl<'a, E, T> Future for MailFuture<'a, T, E>
    where T: BuilderContext,
          E: MailEncoder
{
    type Item = EncodableMail<E>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut done = true;
        let ctx: &T = &self.ctx;
        self.mail.as_mut()
            // this is conform with how futures work, as calling poll on a random future
            // after it completes has unpredictable results (through one of NotReady/Err/Panic)
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
            EncodableMail::from_loaded_mail( self.mail.take().unwrap() )
                .map( |enc_mail| Async::Ready(enc_mail) )
        } else {
            Ok( Async::NotReady )
        }
    }
}

impl<E> EncodableMail<E>
    where E: MailEncoder
{
    fn from_loaded_mail(mut mail: Mail<E>) -> Result<Self> {
        Self::insert_generated_headers(&mut mail)?;
        mail.headers.use_contextual_validators()?;
        if !mail.headers.contains_header(Date) {
            bail!("a mail must have a Date header field");
        }
        if !mail.headers.contains_header(From) {
            bail!("a mail must have a From header field");
        }
        if !mail.headers.contains_header(MessageId) {
            //TODO warn
        }
        Ok(EncodableMail(mail))
    }

    fn insert_generated_headers(mail: &mut Mail<E>) -> Result<()> {
        if let MailPart::SingleBody { ref body } = mail.body {
            let file_buffer = body.get_if_encoded()?
                .expect( "encoded mail, should only contain already transferencoded resources" );

            mail.headers.insert(ContentType, file_buffer.content_type().clone())?;
            mail.headers.insert(ContentTransferEncoding, file_buffer.transfer_encoding().clone())?;
        }

        if !mail.headers.contains_header(Date) {
            mail.headers.insert(Date, DateTime::now())?;
        }
        Ok(())
    }
}

impl<E> Deref for EncodableMail<E>
    where E: MailEncoder
{
    type Target = Mail<E>;
    fn deref( &self ) -> &Self::Target {
        &self.0
    }
}

impl<E> Into<Mail<E>> for EncodableMail<E>
    where E: MailEncoder
{
    fn into( self ) -> Mail<E> {
        self.0
    }
}

impl<E> MailEncodable<E> for EncodableMail<E>
    where E: MailEncoder
{

    fn encode(&self, encoder: &mut E) -> Result<()> {
        // does not panic as a EncodableMail only is constructed from
        // a Mail which has all of it's bodies resolved, without failure
        encode::encode_mail( &self, true, encoder )
    }
}

impl<E> fmt::Debug for EncodableMail<E>
    where E: MailEncoder
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "EncodableMail {{ .. }}")
    }
}