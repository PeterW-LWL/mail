use std::borrow::Cow;
use std::marker::PhantomData;
use vec1::Vec1;

use headers::{
    HeaderMap, HeaderTryFrom,
    _From, _To, Subject, Sender
};
use headers::components::Unstructured;
use mail::{Mail, Builder, Context};

use ::resource::{
    Embedded, EmbeddedWithCId,
    InspectEmbeddedResources, Disposition
};
use ::builder_extension::{
    BodyPart, BuilderExt
};
use ::template_engine::{
    TemplateEngine, MailParts
};
use ::error::CompositionError;

use super::MailSendData;

pub(crate) fn compose_mail<'a, C, E, D>(
    ctx: &C,
    engine: &E,
    send_data: MailSendData<'a, E::TemplateId, D>
) -> Result<Mail, CompositionError<E::Error>>
    where C: Context, E: TemplateEngine<C, D>, D: InspectEmbeddedResources
{
    (Ctx { ctx, engine, _p: PhantomData }).compose_mail(send_data)
}

struct Ctx<'a, 'b, C: 'a, E: 'b, D>
    where C: Context, E: TemplateEngine<C, D>, D: InspectEmbeddedResources
{
    ctx: &'a C,
    engine: &'b E,
    _p: PhantomData<D>
}

impl<'a, 'b, C: 'a, E: 'b, D> Copy for Ctx<'a, 'b, C, E, D>
    where C: Context, E: TemplateEngine<C, D>, D: InspectEmbeddedResources
{}

impl<'a, 'b, C: 'a, E: 'b, D> Clone for Ctx<'a, 'b, C, E, D>
    where C: Context, E: TemplateEngine<C, D>, D: InspectEmbeddedResources
{
    fn clone(&self) -> Self {
        Ctx { ctx: self.ctx, engine: self.engine, _p: self._p }
    }
}

impl<'a, 'b, C, E, D> Ctx<'a, 'b, C, E, D>
    where C: Context, E: TemplateEngine<C, D>, D: InspectEmbeddedResources
{

    fn compose_mail(self, send_data: MailSendData<E::TemplateId, D>)
        -> Result<Mail, CompositionError<E::Error>>
    {
        //compose display name => create Address with display name;
        let (core_headers, data, template_id) = self.process_mail_send_data(send_data)?;

        let MailParts { alternative_bodies, shared_embeddings, attachments }
            = self.use_template_engine(&*template_id, data)?;

        let mail = self.build_mail(alternative_bodies, shared_embeddings.into_iter(),
                                    attachments, core_headers)?;

        Ok(mail)
    }

    fn process_mail_send_data<'n>(
        self,
        send_data:
            MailSendData<'n, E::TemplateId, D>
    ) -> Result<(
        HeaderMap,
        D,
        Cow<'n, E::TemplateId>
    ), CompositionError<E::Error>>
        where D: InspectEmbeddedResources
    {
        let (sender, from_mailboxes, to_mailboxes, subject, template_id, data)
            = send_data.destruct();

        // The subject header field
        let subject = Unstructured::try_from( subject )?;

        // creating the header map
        let mut core_headers: HeaderMap = headers! {
            //NOTE: if we support multiple mailboxes in _From we have to
            // ensure Sender is used _iff_ there is more than one from
            _From: from_mailboxes,
            _To: to_mailboxes,
            Subject: subject
        }?;

        // add sender header if needed
        if let Some(sender) = sender {
            core_headers.insert(Sender, sender)?;
        }

        Ok((core_headers, data, template_id))
    }

    fn use_template_engine(
        self,
        template_id: &E::TemplateId,
        //TODO change to &D?
        data: D
    ) -> Result<MailParts, CompositionError<E::Error>>
        where D: InspectEmbeddedResources
    {
        let mut data = data;
        let mut embeddings = Vec::new();
        let mut attachments = Vec::new();

        data.inspect_resources_mut(&mut |embedded: &mut Embedded| {
            let embedded_wcid = embedded.assure_content_id_and_copy(self.ctx);
            match embedded_wcid.disposition() {
                Disposition::Inline => embeddings.push(embedded_wcid),
                Disposition::Attachment =>  attachments.push(embedded_wcid)
            }
        });

        let mut mail_parts = self.engine
            .use_template(template_id, &data, self.ctx)
            .map_err(|err| CompositionError::Template(err))?;

        mail_parts.attachments.extend(attachments);
        mail_parts.shared_embeddings.extend(embeddings);
        Ok(mail_parts)
    }



    /// uses the results of preprocessing data and templates, as well as a list of
    /// mail headers like `_From`,`To`, etc. to create a new mail
    fn build_mail<EMB>(
        self,
        bodies: Vec1<BodyPart>,
        embeddings: EMB,
        attachments: Vec<EmbeddedWithCId>,
        core_headers: HeaderMap
    ) -> Result<Mail, CompositionError<E::Error>>
        where EMB: Iterator<Item=EmbeddedWithCId> + ExactSizeIterator
    {
        let mail = match attachments.len() {
            0 => Builder::create_alternate_bodies_with_embeddings(
                bodies, embeddings, Some(core_headers))?,
            _n => Builder::create_with_attachments(
                Builder::create_alternate_bodies_with_embeddings(bodies, embeddings, None)?,
                attachments,
                Some(core_headers)
            )?
        };
        Ok(mail)
    }
}
