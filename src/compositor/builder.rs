use std::borrow::{ToOwned, Cow};
use std::fmt::{self, Debug};

use vec1::Vec1;

use headers::components::{
    Mailbox, MailboxList,
};

use ::resource::InspectEmbeddedResources;
use ::error::{MailSendDataError, MailSendDataErrorKind, WithSource, WithSourceExt};

use super::MailSendData;

/// Builder to create `MailSendData`
pub struct MailSendDataBuilder<'a, TId: ?Sized + 'a, D>
    where TId: ToOwned + Debug, TId::Owned: Debug, D: InspectEmbeddedResources + Debug
{
    sender: Option<Mailbox>,
    from: Vec<Mailbox>,
    to: Vec<Mailbox>,
    subject: Option<String>,
    template_id: Option<Cow<'a, TId>>,
    data: Option<D>
}





impl<'a, TId: ?Sized + 'a, D> Debug for MailSendDataBuilder<'a, TId, D>
    where TId: ToOwned + Debug, TId::Owned: Debug, D: InspectEmbeddedResources + Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_struct("MailSendData")
            .field("sender", &self.sender)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("subject", &self.subject)
            .field("template_id", &self.template_id)
            .field("data", &self.data)
            .finish()
    }
}

// Sadly I can not used derive(Default) (it want's a bound on TId)
// if the deriviate create is stable, I could use them for that
impl<'a, TId: ?Sized + 'a, D> Default for MailSendDataBuilder<'a, TId, D>
    where TId: ToOwned + Debug, TId::Owned: Debug, D: InspectEmbeddedResources + Debug
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, TId: ?Sized + 'a, D> MailSendDataBuilder<'a, TId, D>
    where TId: ToOwned + Debug, TId::Owned: Debug, D: InspectEmbeddedResources + Debug
{
    pub fn new() -> Self {
        MailSendDataBuilder {
            sender: None,
            from: Vec::new(),
            to: Vec::new(),
            subject: None,
            template_id: None,
            data: None
        }
    }

    /// adds a Mailbox to the list of from addresses
    pub fn add_from(&mut self, mb: Mailbox) -> &mut Self {
        self.from.push(mb);
        self
    }

    /// add a Mailbox to the list of to addresses
    pub fn add_to(&mut self, mb: Mailbox) -> &mut Self {
        self.to.push(mb);
        self
    }

    /// set the sender to the given mailbox and inserts it into the front of the from Mailboxes
    ///
    /// If a sender was set before it will be override, _but it still will be in the
    /// from MailboxList_.
    pub fn sender(&mut self, mb: Mailbox) -> &mut Self {
        self.sender = Some(mb.clone());
        self.from.insert(0, mb);
        self
    }

    /// sets the subject as a string
    ///
    /// If a subject was set previously it will be overwritten.
    pub fn subject<I>(&mut self, sbj: I) -> &mut Self
        where I: Into<String>
    {
        self.subject = Some(sbj.into());
        self
    }

    /// sets the template_id (borrowed form)
    ///
    /// If a template_id was set previously it will be overwritten.
    pub fn template(&mut self, tid: &'a TId) -> &mut Self {
        self.template_id = Some(Cow::Borrowed(tid));
        self
    }

    /// sets the template_id (owned form)
    ///
    /// If a template_id was set previously it will be overwritten.
    pub fn owned_template(&mut self, tid: <TId as ToOwned>::Owned) -> &mut Self {
        self.template_id = Some(Cow::Owned(tid));
        self
    }

    /// sets the template_id (cow form)
    ///
    /// If a template_id was set previously it will be overwritten.
    pub fn cow_template(&mut self, tid: Cow<'a, TId>) -> &mut Self {
        self.template_id = Some(tid);
        self
    }


    /// sets the data
    ///
    /// If data was set previously it will be overwritten.
    pub fn data(&mut self, data: D) -> &mut Self {
        self.data = Some(data);
        self
    }

    //TODO provide custom error
    /// create `MailSendData` from this builder if possible.
    ///
    /// If there is only one mailbox in from no sender needs
    /// to be set.
    ///
    /// # Error
    ///
    /// Cases in which an error is returned:
    ///
    /// - no data, template_id, from or to was set
    /// - more than one from was set, but no sender was set
    pub fn build(self)
        -> Result<MailSendData<'a, TId, D>, WithSource<MailSendDataError, Self>>
    {
        match self.check_fields_are_set() {
            Ok(_) => {},
            Err(err) => return Err(err.with_source(self))
        }

        if self.from.len() > 1 && self.sender.is_none() {
            return Err(MailSendDataError
                ::from(MailSendDataErrorKind::MultiFromButNoSender)
                .with_source(self));
        }


        //UNWRAP_SAFE..: we already checked that there is data
        let from = Vec1::from_vec(self.from).unwrap();
        let to = Vec1::from_vec(self.to).unwrap();
        let subject = self.subject.unwrap();
        let template_id = self.template_id.unwrap();
        let data = self.data.unwrap();

        Ok(MailSendData {
            sender: self.sender,
            from: MailboxList(from),
            to: MailboxList(to),
            subject,
            template_id,
            data
        })
    }

    fn check_fields_are_set(&self) -> Result<(), MailSendDataError> {
        use self::MailSendDataErrorKind::*;
        let kind =
            if self.from.is_empty() {
                MissingFrom
            } else if self.to.is_empty() {
                MissingTo
            } else if self.subject.is_none() {
                MissingSubject
            } else if self.template_id.is_none() {
                MissingTemplateId
            } else if self.data.is_none() {
                MissingTemplateData
            } else {
                return Ok(());
            };

        Err(MailSendDataError::from(kind))
    }
}
