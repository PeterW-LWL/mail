use std::marker::PhantomData;

use serde::Serialize;
use soft_ascii_string::SoftAsciiStr;

use core::utils::HeaderTryFrom;
use core::error::{Result, ErrorKind, ResultExt};
use core::header::HeaderMap;
use mheaders::{
    From, To,
    Subject,
    ContentId,
    ContentDisposition
};

use mheaders::components::{
    Disposition,
    Unstructured,
    Mailbox,
    Phrase,
};
use mail::mime::gen_multipart_mime;
use mail::{
    Resource,
    Mail,
    Builder
};


pub use self::context::*;
pub use self::templates::*;
pub use self::resource::*;


mod context;
mod templates;
mod resource;

pub type BodyWithEmbeddings = (Resource, Vec<EmbeddingWithCID>);


pub trait NameComposer<D> {
    fn compose_name( &self, data: &D ) -> Option<String>;
}

pub struct Compositor<T, C, CP, D> {
    template_engine: T,
    context: C,
    name_composer: CP,
    _d: PhantomData<D>
}


impl<T, C, CP, D> Compositor<T, C, CP, D>
    where T: TemplateEngine<C>,
          C: Context,
          CP: NameComposer<D>,
          D: Serialize
{
    pub fn new( template_engine: T, context: C, name_composer: CP ) -> Self {
        Compositor { template_engine, context, name_composer, _d: PhantomData }
    }

    /// composes a mail based on the given template_id, data and send_context
    pub fn compose_mail( &self,
                         send_context: MailSendContext,
                         template_id: T::TemplateId,
                         data: D,
    ) -> Result<Mail> {

        let mut data = data;
        //compose display name => create Address with display name;
        let ( subject, from_mailbox, to_mailbox ) =
            self.preprocess_send_context( send_context, &mut data )?;

        let core_headers = headers! {
            //NOTE: if we support multiple mailboxes in From we have to
            // ensure Sender is used _iff_ there is more than one from
            From: (from_mailbox,),
            To: (to_mailbox,),
            Subject: subject
        }?;

        let (bodies, embeddings, attachments) = self.use_template_engine( template_id, data )?;

        self.build_mail( bodies, embeddings, attachments, core_headers )
    }

    pub fn use_template_engine( &self, template_id: T::TemplateId, data: D )
        -> Result<( Vec<BodyWithEmbeddings>, Vec<EmbeddingWithCID>, Vec<Attachment> )>
    {
        let ( (bodies, mut attachments), embeddings, attachments2 ) =
            with_resource_sidechanel( Box::new(self.context.clone()), || -> Result<_> {
                // we just want to make sure that the template engine does
                // really serialize the data, so we make it so that it can
                // only do so (if we pass in the data directly it could use
                // TypeID+Transmut or TraitObject+downcast to undo the generic
                // type erasure and then create the template in some other way
                // but this would break the whole Embedding/Attachment extraction )
                let sdata = SerializeOnly::new( data );
                self.preprocess_templates(
                    self.template_engine
                        .templates( &self.context, template_id, sdata)
                        .chain_err( || "failure in template engine" )?
                        .into() )
            } )?;

        attachments.extend( attachments2 );

        Ok( ( bodies, embeddings, attachments) )
    }

    /// converts To into a mailbox by composing a display name if nessesary,
    /// and converts the String subject into a "Unstructured" text
    /// returns (subjcet, from_mail, to_mail)
    pub fn preprocess_send_context( &self, sctx: MailSendContext, data: &mut D )
        -> Result<(Unstructured, Mailbox, Mailbox)>
    {
        let from_mailbox = sctx.from;
        let to_mailbox = {
            let mut to_mailbox = sctx.to;
            if to_mailbox.display_name.is_none() {
                if let Some( new_name ) = self.name_composer.compose_name( data ) {
                    let phrase = Phrase::try_from( new_name )?;
                    to_mailbox.display_name = Some( phrase );
                }
            }
            to_mailbox
        };
        let subject = Unstructured::try_from( sctx.subject )?;
        //TODO implement some replacement
//        data.see_from_mailbox( &from_mailbox );
//        data.see_to_mailbox( &to_mailbox );
        Ok( ( subject, from_mailbox, to_mailbox ) )
    }




    /// maps all alternate bodies (templates) to
    /// 1. a single list of attachments as they are not body specific
    /// 2. a list of Resource+Embedding pair representing the different (sub-) bodies
    pub fn preprocess_templates( &self, templates: Vec<Template> )
        -> Result<(Vec<BodyWithEmbeddings>, Attachments)>
    {
        let mut bodies = Vec::new();
        let mut attachments = Vec::new();
        for template in templates {
            let mut with_cid = Vec::with_capacity( template.embeddings.len() );
            for embedding in template.embeddings.into_iter() {
                with_cid.push( embedding.with_cid_assured( &self.context )? )
            }

            bodies.push( (template.body, with_cid) );
            attachments.extend( template.attachments );
        }
        Ok( (bodies, attachments) )
    }


    /// uses the results of preprocessing data and templates, as well as a list of
    /// mail headers like `From`,`To`, etc. to create a new mail
    pub fn build_mail( &self,
                       bodies: Vec<BodyWithEmbeddings>,
                       embeddings: Vec<EmbeddingWithCID>,
                       attachments: Attachments,
                       core_headers: HeaderMap
    ) -> Result<Mail> {
        let mail = match attachments.len() {
            0 => Builder::create_alternate_bodies_with_embeddings(
                bodies, embeddings, Some(core_headers) )?,
            _n => Builder::create_with_attachments(
                Builder::create_alternate_bodies_with_embeddings(bodies, embeddings, None )?,
                attachments,
                Some( core_headers )
            )?
        };
        Ok( mail )
    }
}




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
        attachments: Attachments,
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
        attachments: Attachments,
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

