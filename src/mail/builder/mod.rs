//  Builder
//     .multipart( MultipartMime ) -> MultipartBuilder
//          .set_header( Header )?
//          .set_body( |builder| builder.singlepart( ... )...build() )?
//          .set_body( |builder| builder.multipart( Mime )...build() )?
//          .build()?
//     .singlepart( Resource ) -> SinglePartBuilder
//          .set_header( Header )
//          .build()
//
//
/// TODO set_** in builder to just **


use ascii::AsciiString;

use error::*;
use utils::is_multipart_mime;
use headers::Header;

use super::mime::MultipartMime;
use super::resource::Resource;
use super::{ MailPart, Mail, Headers };

#[cfg(test)]
mod test_utils;

use super::context::*;

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

        self.inner.build( MailPart::SingleBody { body: self.body } )
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
