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
use ascii::AsciiString;

use utils::uneraser_ref;
use error::*;
use codec::{ MailEncoder, MailEncodable };
use utils::{ is_multipart_mime, HeaderTryInto };
use headers::{
    HeaderMap, Header,
    ContentType,
    ContentTransferEncoding
};
use components::Mime;

use super::mime::MultipartMime;
use super::resource::Resource;
use super::{ MailPart, Mail };


pub struct Builder;

struct BuilderShared<E: MailEncoder> {
    headers: HeaderMap<E>
}

pub struct SinglepartBuilder<E: MailEncoder> {
    inner: BuilderShared<E>,
    body: Resource
}

pub struct MultipartBuilder<E: MailEncoder> {
    inner: BuilderShared<E>,
    hidden_text: Option<AsciiString>,
    bodies: Vec<Mail<E>>
}

impl<E> BuilderShared<E> where E: MailEncoder {

    fn new() -> Self {
        BuilderShared {
            headers: HeaderMap::new()
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
    fn header<H>(
        &mut self,
        header: H,
        hbody: H::Component,
        is_multipart: bool
    ) -> Result<()>
        where H: Header,
              H::Component: MailEncodable<E>
    {
        check_header::<H, _>(&hbody, is_multipart)?;
        self.headers.insert( header, hbody )
    }

    fn headers( &mut self, headers: HeaderMap<E>, is_multipart: bool ) -> Result<()> {
        check_multiple_headers( &headers, is_multipart )?;
        self.headers.extend( headers )?;
        Ok( () )
    }

    fn build( self, body: MailPart<E> ) -> Result<Mail<E>> {
        Ok( Mail {
            headers: self.headers,
            body: body,
        } )
    }
}

pub fn check_multiple_headers<E>( headers: &HeaderMap<E> , is_multipart: bool) -> Result<()>
    where E: MailEncoder
{
    if let Some( .. ) = headers.get_single::<ContentTransferEncoding>() {
        bail!( concat!(
            "setting content transfer encoding through a header is not supported,",
            "use Ressource::set_preferred_encoding on the body instead"
        ) );
    }
    if let Some( mime ) = headers.get_single::<ContentType>() {
        if is_multipart != is_multipart_mime( mime? ) {
            return Err( ErrorKind::ContentTypeAndBodyIncompatible.into() )
        }
    }
    Ok( () )
}

pub fn check_header<H, E>(
    hbody: &H::Component,
    is_multipart: bool
) -> Result<()>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    match H::name().as_str() {
        "Content-Type" => {
            let mime: &Mime = uneraser_ref(hbody)
                .ok_or_else( || "custom Content-Type headers are not supported" )?;
            if is_multipart != is_multipart_mime( mime ) {
                return Err( ErrorKind::ContentTypeAndBodyIncompatible.into() )
            }
        },
        "Content-Transfer-Encoding" => {
            bail!( concat!(
                "setting content transfer encoding through a header is not supported,",
                "use Ressource::set_preferred_encoding on the body instead"
            ) );
        }
        _ => {}
    }
    Ok( () )
}

impl Builder {

    pub fn multipart<E: MailEncoder>( mime: MultipartMime ) -> MultipartBuilder<E> {
        let res = MultipartBuilder {
            inner: BuilderShared::new(),
            hidden_text: None,
            bodies: Vec::new(),
        };

        //UNWRAP_SAFETY: it can only fail with illegal headers,
        // but this header can not be illegal
        res.header( ContentType, mime ).unwrap()
    }

    pub fn singlepart<E: MailEncoder>( r: Resource ) -> SinglepartBuilder<E> {
        SinglepartBuilder {
            inner: BuilderShared::new(),
            body: r,
        }
    }

}

impl<E> SinglepartBuilder<E>
    where E: MailEncoder
{

    pub fn header<H, C>(
        &mut self,
        header: H,
        hbody: C
    ) -> Result<()>
        where H: Header,
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        let comp = hbody.try_into()?;
        self.inner.header( header, comp, false )
    }

    pub fn headers( mut self, headers: HeaderMap<E> ) -> Result<Self> {
        self.inner.headers( headers, false )?;
        Ok( self )
    }

    pub fn build( self ) -> Result<Mail<E>> {

        self.inner.build( MailPart::SingleBody { body: self.body } )
    }
}

impl<E> MultipartBuilder<E>
    where E: MailEncoder
{


    ///
    /// # Error
    ///
    /// A error is returned if the header is incompatible with this builder,
    /// i.e. if a ContentType header is set with a non-multipart content type
    pub fn header<H, C>(
        mut self,
        header: H,
        hbody: C
    ) -> Result<Self>
        where H: Header,
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        let comp = hbody.try_into()?;
        self.inner.header( header, comp, true )?;
        Ok( self )
    }

    pub fn headers( mut self, headers: HeaderMap<E> ) -> Result<Self> {
        self.inner.headers( headers, true )?;
        Ok( self )
    }

    pub fn body( mut self, body: Mail<E> ) -> Result<Self> {
        self.bodies.push( body );
        Ok( self )
    }

    pub fn build( self ) -> Result<Mail<E>> {
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

//TODO test
// - can not misset Content-Type
// - can not set Content-Transfer-Encoding (done through ressource)
// - above tests but wrt. set_headers/headers