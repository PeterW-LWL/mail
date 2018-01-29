use std::marker::PhantomData;

use serde::Serialize;

use core::utils::HeaderTryFrom;
use core::error::{Result, ResultExt};
use core::header::HeaderMap;
use headers::{From, To, Subject};
use headers::components::{Unstructured, Mailbox, Phrase};
use mail::{Mail, Builder};

use utils::SerializeOnly;
use context::{Context, MailSendContext};
use resource::{
    EmbeddingWithCID, Attachment,
    BodyWithEmbeddings,
    with_resource_sidechanel
};
use builder_extension::BuilderExt;
use template::{
    Template, TemplateEngine
};

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
                                 -> Result<(Vec<BodyWithEmbeddings>, Vec<Attachment>)>
    {
        let mut bodies = Vec::new();
        let mut attachments = Vec::new();
        for template in templates {
            let embeddings = template.embeddings.into_iter()
                .map(|embedding| embedding.with_cid_assured(&self.context))
                .collect::<Result<Vec<_>>>()?;

            bodies.push( (template.body, embeddings) );
            attachments.extend( template.attachments );
        }
        Ok( (bodies, attachments) )
    }


    /// uses the results of preprocessing data and templates, as well as a list of
    /// mail headers like `From`,`To`, etc. to create a new mail
    pub fn build_mail( &self,
                       bodies: Vec<BodyWithEmbeddings>,
                       embeddings: Vec<EmbeddingWithCID>,
                       attachments: Vec<Attachment>,
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



