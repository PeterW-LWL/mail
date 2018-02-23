use std::marker::PhantomData;
use std::borrow::Cow;

use serde::Serialize;
use vec1::Vec1;

use core::utils::HeaderTryFrom;
use core::error::{Result, ResultExt};
use core::header::HeaderMap;
use headers::{From, To, Subject, Sender};
use headers::components::{
    Unstructured,
    Mailbox, MailboxList,
    Phrase,
    Email
};
use mail::{Mail, Builder};

use utils::SerializeOnly;
use context::{Context, MailSendData};
use resource::{
    EmbeddingWithCId, Attachment,
    with_resource_sidechanel
};
use builder_extension::BuilderExt;
use template::{
    BodyPart, TemplateEngine, MailParts
};

pub trait NameComposer<D> {
    /// generates a display name used in a From header based on email address and mails data
    ///
    /// The data is passed in as a `&mut` ref so that the generated name can
    /// also be made available to the template engine, e.g. for generating
    /// greetings. The data should _not_ be changed in any other way.
    ///
    /// The composer can decide to not generate a display name if, e.g. there
    /// is not enough information to doe so.
    ///
    /// # Error
    ///
    /// A error can be returned if generated the name failed, e.g. because
    /// a query to a database failed with an connection error. A error should
    /// _not_ be returned if there is "just" not enough data to create a display
    /// name, in which `Ok(None)` should be returned indicating that there is
    /// no display name.
    fn compose_from_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>>;

    /// generates a display name used in a To header based on email address and mails data
    /// The data is passed in as a `&mut` ref so that the generated name can
    /// also be made available to the template engine, e.g. for generating
    /// greetings. The data should _not_ be changed in any other way.
    ///
    /// The composer can decide to not generate a display name if, e.g. there
    /// is not enough information to doe so.
    ///
    /// # Error
    ///
    /// A error can be returned if generated the name failed, e.g. because
    /// a query to a database failed with an connection error. A error should
    /// _not_ be returned if there is "just" not enough data to create a display
    /// name, in which `Ok(None)` should be returned indicating that there is
    /// no display name.
    fn compose_to_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>>;

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
    pub fn compose_mail(&self, send_data: MailSendData<T::TemplateId, D>)
        -> Result<Mail>
    {

        //compose display name => create Address with display name;
        let (core_headers, data, template_id) = self.process_mail_send_data(send_data)?;

        let MailParts { alternative_bodies, shared_embeddings, attachments }
            = self.use_template_engine(&*template_id, data)?;

        self.build_mail( alternative_bodies, shared_embeddings.into_iter(), attachments,
                         core_headers )
    }

    pub fn process_mail_send_data<'a>(&self, send_data: MailSendData<'a, T::TemplateId, D>)
        -> Result<(HeaderMap, D, Cow<'a, T::TemplateId>)>
    {
        let mut send_data = send_data;
        // we need to set a sender if we have more than one sender/from
        let sender = if !send_data.other_from.is_empty() {
            Some(send_data.sender.clone())
        } else {
            None
        };

        // the sender is the first from (compositor for now does not support a sender which
        // is not in the from list)
        let from_mailboxes = self.prepare_mailboxes(
            Some(send_data.sender).into_iter().chain(send_data.other_from.into_iter()),
            &mut send_data.data,
            true
        )?;

        // the To MailboxList
        let to_mailboxes = self.prepare_mailboxes(
            send_data.to,
            &mut send_data.data,
            false
        )?;

        // The subject header field
        let subject = Unstructured::try_from( send_data.subject )?;

        // createing the header map
        let mut core_headers: HeaderMap = headers! {
            //NOTE: if we support multiple mailboxes in From we have to
            // ensure Sender is used _iff_ there is more than one from
            From: from_mailboxes,
            To: to_mailboxes,
            Subject: subject
        }?;

        // add sender header if needed
        if let Some(sender) = sender {
            core_headers.insert(Sender, sender)?;
        }

        Ok((core_headers, send_data.data, send_data.template_id))
    }

    pub fn use_template_engine( &self, template_id: &T::TemplateId, data: D )
                                -> Result<MailParts>
    {
        let id_gen = Box::new(self.context.clone());
        let ( mut mail_parts, embeddings, attachments ) =
            with_resource_sidechanel(id_gen, || -> Result<_> {
                // we just want to make sure that the template engine does
                // really serialize the data, so we make it so that it can
                // only do so (if we pass in the data directly it could use
                // TypeID+Transmut or TraitObject+downcast to undo the generic
                // type erasure and then create the template in some other way
                // but this would break the whole Embedding/Attachment extraction )
                let sdata = SerializeOnly::new(data);
                self.template_engine
                    .use_templates(&self.context, template_id, &sdata)
                    .chain_err(|| "failure in template engine")
            })?;

        mail_parts.attachments.extend(attachments);
        mail_parts.shared_embeddings.extend(embeddings);
        Ok(mail_parts)
    }

    /// creates a MailboxList with default display_names from a non empty sequence of Mailboxes
    ///
    /// # Panics
    ///
    /// if the input was an empty sequence of mailboxes
    fn prepare_mailboxes<I>(&self, non_empty_seq: I, data: &mut D, from: bool) -> Result<MailboxList>
        where I: IntoIterator<Item=Mailbox>
    {
        let vec = non_empty_seq.into_iter()
            .map(|mailbox| mailbox.with_default_name( |email| {
                let res = if from {
                    self.name_composer.compose_from_name(email, data)
                } else {
                    self.name_composer.compose_to_name(email, data)
                };

                match res? {
                    Some(name) => Ok(Some(Phrase::try_from(name)?)),
                    None => Ok(None)
                }
            }))
            .collect::<Result<Vec<_>>>()?;

        //UNWRAP_SAFE: only panics if to_mailbox len == 0, but it's created from to
        // which has len > 0 enforced at type level and only map+collect was used
        Ok(MailboxList(Vec1::from_vec(vec).unwrap()))
    }

    /// uses the results of preprocessing data and templates, as well as a list of
    /// mail headers like `From`,`To`, etc. to create a new mail
    pub fn build_mail<EMB>(&self,
                           bodies: Vec1<BodyPart>,
                           embeddings: EMB,
                           attachments: Vec<Attachment>,
                           core_headers: HeaderMap
    ) -> Result<Mail>
        where EMB: Iterator<Item=EmbeddingWithCId> + ExactSizeIterator
    {
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



