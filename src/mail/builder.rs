//  Builder
//     .multipart( MultipartMime ) -> MultipartBuilder
//          .add_header( Header )
//          .add_body( |builder| builder.singlepart( ... )...build() )
//          .add_body( |builder| builder.multipart( Mime )...build() )
//          .build()
//     .singlepart( Resource ) -> SinglePartBuilder
//          .add_header( Header )
//          .build()
//
//
//


use std::sync::Arc;
use std::path::Path;
use std::ops::Deref;

use ascii::AsciiString;
use futures::{ Future, IntoFuture };
use futures::future::{ self,  BoxFuture };

use error::*;
use types::buffer;
use utils::is_multipart_mime;
use headers::Header;
use codec::transfer_encoding::TransferEncodedFileBuffer;

use super::mime::MultipartMime;
use super::resource::Resource;
use super::{ MailPart, Mail, Headers, Body };


pub trait FileLoader {
    type FileFuture: Future<Item=Vec<u8>, Error=Error> + Send + 'static;
    /// load file specified by path, wile it returns
    /// a future it is not required to load the file
    /// in the background, as such you should not relay
    /// on it beeing non-blocking, it might just load
    /// the file in place and return futures::ok
    fn load_file( &self, path: &Path ) -> Self::FileFuture;
}

impl<F: FileLoader> FileLoader for Arc<F> {
    type FileFuture = F::FileFuture;
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        self.deref().load_file( path )
    }
}

pub trait RunElsewhere {
    /// executes the futures `fut` "elswhere" e.g. in a cpu pool
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static;

    fn execute_fn<FN, I>( &self, fut: FN ) -> BoxFuture<I::Item, I::Error>
        where FN: FnOnce() -> I + Send + 'static,
              I: IntoFuture + 'static,
              I::Future: Send + 'static,
              I::Item: Send + 'static,
              I::Error: Send + 'static
    {
        self.execute( future::lazy( fut ) )
    }
}

impl<I: RunElsewhere> RunElsewhere for Arc<I> {
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.deref().execute( fut )
    }
}

pub trait BuilderContext: FileLoader + RunElsewhere + Clone + Send + 'static {}
impl<T> BuilderContext for T where T: FileLoader+RunElsewhere+Clone+Send + 'static{}


pub struct Builder<E: BuilderContext>(pub E);

struct BuilderShared<E: BuilderContext> {
    ctx: E,
    headers: Headers
}

pub struct SinglepartBuilder<E: BuilderContext> {
    inner: BuilderShared<E>,
    body: Resource
}

pub struct MultipartBuilder<E: BuilderContext> {
    inner: BuilderShared<E>,
    hidden_text: Option<AsciiString>,
    bodies: Vec<Mail>
}

impl<E: BuilderContext> BuilderShared<E> {

    fn new( ctx: E ) -> Self {
        BuilderShared {
            ctx,
            headers: Headers::new(),
        }
    }


    ///
    /// # Error
    ///
    /// A error is returned if the header is incompatible with this builder,
    /// i.e. if a ContentType header is set with a non-multipart content type
    /// is set on a multipart mail or a multipart content type is set on a
    /// non-mutltipart mail
    ///
    /// NOTE: do NOT add other error cases
    fn set_header( &mut self, header: Header, is_multipart: bool ) -> Result<Option<Header>> {
        use headers::Header::*;
        //move checks for single/multipart from mail_composition here
        match &header {
            //FIXME check if forbidding setting ContentType/ContentTransferEncoding headers
            // is preferable, especially if is_multipart == false
            &ContentType( ref mime ) => {
                if is_multipart != is_multipart_mime( mime ) {
                    return Err( ErrorKind::ContentTypeAndBodyIncompatible.into() )
                }
            },
            _ => {}
        }

        let name = header.name().into();

        Ok( self.headers.insert( name, header ) )
    }

    fn set_headers<IT>( &mut self, iter: IT, is_multipart: bool ) -> Result<()>
        where IT: IntoIterator<Item=Header>
    {
        for header in iter.into_iter() {
            self.set_header( header, is_multipart )?;
        }
        Ok( () )
    }

    fn build( self, body: MailPart ) -> Result<Mail> {
        Ok( Mail {
            headers: self.headers,
            body: body,
        } )
    }
}

impl<E: BuilderContext> Builder<E> {

    pub fn multipart( &self,  m: MultipartMime ) -> MultipartBuilder<E> {
        let res = MultipartBuilder {
            inner: BuilderShared::new( self.0.clone() ),
            hidden_text: None,
            bodies: Vec::new(),
        };

        //UNWRAP_SAFETY: it can only fail with illegal headers, but this header can not be illegal
        res.set_header( Header::ContentType( m.into() ) ).unwrap()
    }

    pub fn singlepart( &self, r: Resource ) -> SinglepartBuilder<E> {
        SinglepartBuilder {
            inner: BuilderShared::new( self.0.clone() ),
            body: r,
        }
    }

}

impl<E: BuilderContext> SinglepartBuilder<E> {
    pub fn set_header( mut self, header: Header ) -> Result<Self> {
        self.inner.set_header( header, false )?;
        Ok( self )
    }

    pub fn set_headers<IT>( mut self, iter: IT ) -> Result<Self>
        where IT: IntoIterator<Item=Header>
    {
        self.inner.set_headers( iter, false )?;
        Ok( self )

    }

    pub fn build( self ) -> Result<Mail> {
        use self::Resource::*;

        let body: Body = match self.body {
            FileBuffer( buffer ) => {
                self.inner.ctx.execute_fn(
                    move || TransferEncodedFileBuffer::encode_buffer( buffer, None )
                ).into()
            },
            Future( future ) => {
                let ctx = self.inner.ctx.clone();
                future.and_then( move |buffer|
                    ctx.execute_fn(
                        move || TransferEncodedFileBuffer::encode_buffer( buffer, None )
                    )
                ).boxed().into()
            },
            File { mime, path, alternate_name } => {
                let _ = alternate_name;
                self.inner.ctx.execute(
                    self.inner.ctx.load_file( &*path ).and_then( |data| {
                        //TODO add file meta, replacing name with alternate_name (if it is some)
                        let buffer = buffer::FileBuffer::new( mime.into(), data );
                        TransferEncodedFileBuffer::encode_buffer( buffer, None )
                    })
                ).into()
            }
        };

        self.inner.build( MailPart::SingleBody { body } )
    }
}

impl<E: BuilderContext> MultipartBuilder<E> {
    pub fn add_body<FN>( mut self, body_fn: FN ) -> Result<Self>
        where FN: FnOnce( &Builder<E> ) -> Result<Mail>
    {
        self.bodies.push( body_fn( &Builder( self.inner.ctx.clone() ) )? );
        Ok( self )
    }

    pub fn set_headers<IT>( mut self, iter: IT ) -> Result<Self>
        where IT: IntoIterator<Item=Header>
    {
        self.inner.set_headers( iter, true )?;
        Ok( self )

    }

    ///
    /// # Error
    ///
    /// A error is returned if the header is incompatible with this builder,
    /// i.e. if a ContentType header is set with a non-multipart content type
    pub fn set_header( mut self, header: Header ) -> Result<Self> {
        self.inner.set_header( header, true )?;
        Ok( self )
    }

    pub fn build( self ) -> Result<Mail> {
        if self.bodies.len() == 0 {
            Err( ErrorKind::NeedAtLastOneBodyInMultipartMail.into() )
        } else {
            self.inner.build( MailPart::MultipleBodies {
                bodies: self.bodies,
                hidden_text: self.hidden_text.unwrap_or( AsciiString::new() ),
            } )
        }
    }
}