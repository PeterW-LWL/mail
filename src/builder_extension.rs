use media_type::{MULTIPART, ALTERNATIVE, RELATED, MIXED};
use vec1::Vec1;

use core::error::{Result, ErrorKind};
use core::header::HeaderMap;

use headers::{ContentId, ContentDisposition};
use headers::components::Disposition;
use mail::MediaType;
use mail::{Resource, Mail, Builder};

use template::BodyPart;
use resource::{EmbeddingWithCId,  Attachment};


/// Ext. Trait which adds helper methods to the Builder type.
///
pub trait BuilderExt {

    fn create_alternate_bodies<HM>(
        bodies: Vec1<BodyPart>,
        header: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>;


    fn create_mail_body<HM>(
        body: BodyPart,
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

    fn create_body_with_embeddings<HM, EMB>(
        sub_body: Mail,
        embeddings: EMB,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>,
              EMB: Iterator<Item=EmbeddingWithCId> + ExactSizeIterator;

    fn create_alternate_bodies_with_embeddings<HM, EMB>(
        bodies: Vec1<BodyPart>,
        embeddings: EMB,
        header: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>,
              EMB: Iterator<Item=EmbeddingWithCId> + ExactSizeIterator;
}



impl BuilderExt for Builder {

    fn create_alternate_bodies<HM>(
        bodies: Vec1<BodyPart>,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        let bodies = bodies;

        match bodies.len() {
            0 => bail!( ErrorKind::NeedAtLastOneBodyInMultipartMail ),
            1 => return Self::create_mail_body(bodies.into_vec().pop().unwrap(), headers ),
            _n => {}
        }

        let mut builder = Builder::multipart(MediaType::new(MULTIPART, ALTERNATIVE)?)?;

        if let Some(headers) = headers.into() {
            builder = builder.headers( headers )?;
        }

        for body in bodies {
            builder = builder.body( Self::create_mail_body( body, None )? )?;
        }

        builder.build()
    }

    fn create_alternate_bodies_with_embeddings<HM, EMB>(
        bodies: Vec1<BodyPart>,
        embeddings: EMB,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>,
              EMB: Iterator<Item=EmbeddingWithCId> + ExactSizeIterator
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
        body: BodyPart,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>
    {
        let BodyPart { body_resource, embeddings } = body;
        if embeddings.len() > 0 {
            Self::create_body_with_embeddings(
                Self::create_body_from_resource( body_resource, None )?,
                embeddings.into_iter(),
                headers
            )
        } else {
            Self::create_body_from_resource( body_resource, headers )
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

    fn create_body_with_embeddings<HM, EMB>(
        sub_body: Mail,
        embeddings: EMB,
        headers: HM
    ) -> Result<Mail>
        where HM: Into<Option<HeaderMap>>,
              EMB: Iterator<Item=EmbeddingWithCId> + ExactSizeIterator
    {

        if embeddings.len() == 0 {
            bail!( "this function except at last one embedding" )
        }

        let mut builder = Builder::multipart( MediaType::new(MULTIPART,RELATED)?)?;

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

        let mut builder = Builder::multipart(MediaType::new(MULTIPART, MIXED)?)?;

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

