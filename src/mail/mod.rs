//FIXME use FnvHashMap
use std::collections::HashMap;
use std::collections::hash_map::{ Iter as MapIter };
use std::borrow::Cow;

use codec::{ MailEncoder, MailEncodable };
use ascii::{ AsciiString, AsciiStr, AsciiChar };
use futures::{ Future, Async, Poll, IntoFuture };

use error::*;
use utils::is_multipart_mime;
use headers::Header;

use self::body::Body;
pub use self::builder::*;

pub mod body;
pub mod resource;
pub mod mime;
mod builder;
mod encode;


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
        body: Body
    },
    MultipleBodies {
        bodies: Vec<Mail>,
        hidden_text: AsciiString
    }
}

pub struct MailFuture( Option<Mail> );

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

    fn walk_mail_bodies_mut<FN>( &mut self, use_it_fn: &mut FN) -> Result<()>
        where FN: FnMut( &mut Body ) -> Result<()>
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




impl IntoFuture for Mail {
    type Future = MailFuture;
    type Item = EncodableMail;
    type Error = Error;

    /// converts the Mail into a future,
    ///
    /// the future resolves once
    /// all contained BodyFutures are resolved (or one of
    /// them resolves into an error in which case it will
    /// resolve to the error and cancel all other BodyFutures)
    ///
    ///
    fn into_future(self) -> Self::Future {
        MailFuture( Some(self) )
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


impl Future for MailFuture {
    type Item = EncodableMail;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut done = true;
        self.0.as_mut()
            // this is conform with how futures work, as calling poll on a random future
            // after it completes has unpredictable results (through one of NotRady/Err/Panic)
            // use `Fuse` if you want more preditable behaviour in this edge case
            .expect( "poll not to be called after completion" )
            .walk_mail_bodies_mut( &mut |body| {
                match body.poll_body() {
                    Ok( None ) => {
                        done = false;
                        Ok(())
                    },
                    Ok( Some(..) ) => {
                        Ok(())
                    },
                    Err( err ) => {
                         Err( err )
                    }
                }
            })?;

        if done {
            Ok( Async::Ready( EncodableMail( self.0.take().unwrap() ) ) )
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

impl MailEncodable for EncodableMail {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        // does not panic as a EncodableMail only is constructed from
        // a Mail which has all of it's bodies resolved, without failure
        encoding::encode_mail( &self.0, true, encoder )
    }
}

mod encoding {
    use super::*;
    use mime::BOUNDARY;
    use ascii::IntoAsciiString;


    ///
    /// # Panics
    /// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
    /// on `Mail` to prevent this from happening
    ///
    pub fn encode_mail<E>( mail: &Mail, top: bool, encoder: &mut E ) -> Result<()>
        where E: MailEncoder
    {

        encode_headers( mail, top, encoder )?;

        //the empty line between the headers and the body
        encoder.write_new_line();

        encode_mail_part( mail, encoder )?;

        Ok( () )
    }

    ///
    /// # Panics
    /// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
    /// on `Mail` to prevent this from happening
    ///
    pub fn encode_headers<E>(mail: &Mail, top: bool, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        let special_headers = find_special_headers( mail );
        let iter = special_headers
            .iter()
            .chain( mail.headers.values() );

        if top {
            encoder.write_str( ascii_str!{ M I M E Minus V e r s i o n Colon Space _1 Dot _0 } );
            encoder.write_new_line();
        }

        for header in iter {
            let ignored_header = !top &&
                !(header.name().as_str().starts_with("Content-")
                    || header.name().as_str().starts_with("X-") );

            if ignored_header {

                //TODO warn!
            }

            header.encode( encoder )?;
            encoder.write_new_line();
        }
        Ok( () )
    }

    //FEATURE_TODO(use_impl_trait): return impl Iterator or similar
    ///
    /// # Panics
    /// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
    /// on `Mail` to prevent this from happening
    ///
    pub fn find_special_headers( mail: &Mail ) -> Vec<Header> {
        let mut headers = vec![];
        //we need: ContentType, ContentTransferEncoding, and ??
        match mail.body {
            MailPart::SingleBody { ref body } => {
                let file_buffer = body.file_buffer_ref().expect( "the body to be resolved" );
                headers.push(
                    Header::ContentType( file_buffer.content_type().clone() ) );
                headers.push(
                    Header::ContentTransferEncoding( file_buffer.transfer_encoding().clone() ) );
            },
            //TODO are there more special headers? (Such which are derived from the body, etc.)
            // yes if there are file_meta we want to replace any ContentDisposition header with
            // our version containing file meta
            //TODO bail if there is a ContentTransferEncoding in a multipart body!
            _ => {}
        }
        headers
    }

    ///
    /// # Panics
    /// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
    /// on `Mail` to prevent this from happening
    ///
    pub fn encode_mail_part<E>(mail: &Mail, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        use super::MailPart::*;
        match mail.body {
            SingleBody { ref body } => {
                if let Some( file_buffer ) = body.file_buffer_ref() {
                    encoder.write_body( file_buffer )
                } else {
                    bail!( "unresolved body" )
                }
            },
            MultipleBodies { ref hidden_text, ref bodies } => {
                if hidden_text.len() > 0 {
                    //TODO warn that encoding hidden text is not implemented for now
                }
                let boundary: String = {
                    //FIXME there has to be a better way
                    if let Some( header ) = mail.headers.get(
                        ascii_str!( C o n t e n t Minus T y p e )
                    ) {
                        match header {
                            &Header::ContentType( ref mime ) => {
                                mime.get_param(BOUNDARY)
                                    .ok_or_else( ||-> Error { "boundary gone missing".into() } )?
                                    .to_string()
                            }
                            _ => bail!( "Content-Type header corrupted" )
                        }
                    } else {
                        bail!( "Content-Type header gone missing" );
                    }
                };

                let boundary = boundary.into_ascii_string().chain_err( || "non ascii boundary" )?;

                for mail in bodies.iter() {
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_str( &*boundary );
                    encoder.write_new_line();

                    encode_mail( mail, false, encoder )?;
                }

                if bodies.len() > 0 {
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_str( &*boundary );
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_char( AsciiChar::Minus );
                    encoder.write_new_line();
                } else {
                    //TODO warn
                }

            }
        }
        Ok( () )
    }
}