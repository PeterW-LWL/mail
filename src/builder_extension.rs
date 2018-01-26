use soft_ascii_string::SoftAsciiStr;

use core::error::{Result, ErrorKind};
use core::header::HeaderMap;

use headers::{ContentId, ContentDisposition};
use headers::components::Disposition;
use mail::mail::mime::gen_multipart_mime;
use mail::{Resource, Mail, Builder};

use resource::{EmbeddingWithCID, BodyWithEmbeddings, Attachment};


/// Ext. Trait which adds helper methods to the Builder type.
///
pub trait BuilderExt {

    fn create_alternate_bodies<HM>(
        bodies: Vec<BodyWithEmbeddings>,
        header: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

    fn create_alternate_bodies_with_embeddings<HM>(
        bodies: Vec<BodyWithEmbeddings>,
        embeddings: Vec<EmbeddingWithCID>,
        header: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

    fn create_mail_body<HM>(
        body: BodyWithEmbeddings,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

    fn create_with_attachments<HM>(
        body: Mail,
        attachments: Vec<Attachment>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

    fn create_body_from_resource<HM>(
        resource: Resource,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

    fn create_body_with_embeddings<HM>(
        sub_body: Mail,
        embeddings: Vec<EmbeddingWithCID>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;

}



impl BuilderExt for Builder {

    fn create_alternate_bodies<HM>(
        bodies: Vec<BodyWithEmbeddings>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        let mut bodies = bodies;

        match bodies.len() {
            0 => bail!( ErrorKind::NeedPlainAndOrHtmlMailBody ),
            1 => return Self::create_mail_body(bodies.pop().unwrap(), headers ),
            _n => {}
        }

        let mut builder = Builder
        ::multipart(gen_multipart_mime(SoftAsciiStr::from_str_unchecked("alternate"))?);

        if let Some(headers) = headers.into() {
            builder = builder.headers( headers )?;
        }

        for body in bodies {
            builder = builder.body( Self::create_mail_body( body, None )? )?;
        }

        builder.build()
    }

    fn create_alternate_bodies_with_embeddings<HM>(
        bodies: Vec<BodyWithEmbeddings>,
        embeddings: Vec<EmbeddingWithCID>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        match embeddings.len() {
            0 => {
                Self::create_alternate_bodies( bodies, headers )
            },
            _n => {
                Self::create_body_with_embeddings(
                    Self::create_alternate_bodies( bodies, None )?,
                    embeddings,
                    headers
                )
            }
        }
    }

    fn create_mail_body<HM>(
        body: BodyWithEmbeddings,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        let (resource, embeddings) = body;
        if embeddings.len() > 0 {
            Self::create_body_with_embeddings(
                Self::create_body_from_resource( resource, None )?,
                embeddings,
                headers
            )
        } else {
            Self::create_body_from_resource( resource, headers )
        }
    }

    fn create_body_from_resource<HM>(
        resource: Resource,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        let mut builder = Builder::singlepart( resource );
        if let Some( headers ) = headers.into() {
            builder = builder.headers( headers )?;
        }
        builder.build()
    }

    fn create_body_with_embeddings<HM>(
        sub_body: Mail,
        embeddings: Vec<EmbeddingWithCID>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {

        if embeddings.len() == 0 {
            bail!( "this function except at last one embedding" )
        }

        let mut builder = Builder
        ::multipart( gen_multipart_mime(
            SoftAsciiStr::from_str_unchecked("related"))? );

        if let Some( headers ) = headers.into() {
            builder = builder.headers( headers )?;
        }


        builder = builder.body( sub_body )?;
        for embedding in embeddings {
            let ( content_id, resource ) = embedding.into();
            builder = builder.body(
                Self::create_body_from_resource( resource , headers! {
                    ContentId: content_id,
                    ContentDisposition: Disposition::inline()
                }? )?
            )?;
        }
        builder.build()
    }


    fn create_with_attachments<HM>(
        body: Mail,
        attachments: Vec<Attachment>,
        headers: HM
    )  -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {

        let mut builder = Builder::multipart(
            gen_multipart_mime( SoftAsciiStr::from_str_unchecked("mixed"))? );

        if let Some( headers ) = headers.into() {
            builder = builder.headers( headers )?;
        }

        builder = builder.body( body )?;

        for attachment in attachments {
            builder = builder.body( Self::create_body_from_resource(
                attachment.into(),
                headers! {
                    ContentDisposition: Disposition::attachment()
                }?
            )? )?;
        }

        builder.build()
    }
}

