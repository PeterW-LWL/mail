use std::borrow::{ToOwned, Cow};
use std::sync::Arc;
use std::ops::Deref;
use std::fmt::{self, Debug};

use serde::Serialize;
use vec1::Vec1;

use error::Error;
use core::HeaderTryFrom;
use mail::headers::components::{
    Mailbox, MailboxList,
    Phrase, Email
};

/// A type containing all per-Mail specific information
///
/// The contained information is:
///
/// - sender (if any)
/// - from (1+ Mailboxes)
/// - to (1+ Mailboxes)
/// - subject (a String)
/// - template (a template id, or more concrete `Cow<'a, TId>`, often a cow string)
/// - data (the data for the template)
///
/// To create a `MailSendData` instance use the `MailSendDataBuilder`.
///
/// All information in a `MailSendData` instance can be accesses for reading,
/// but some constraints are set on modifying it so that following constraints
/// are uphold:
///
/// 1. if there is more than one Mailbox in from then there is a sender
/// 2. there has to be at last one Mailbox in from
/// 3. there has to be at last one Mailbox in to
///
/// # Example (Construction)
///
/// ```
/// # extern crate mail_codec as mail;
/// # extern crate mail_codec_composition as compose;
/// # extern crate mail_codec_core as core;
/// # use std::collections::HashMap;
/// # use compose::MailSendDataBuilder;
/// # use mail::headers::components::{Mailbox, Email};
/// # use core::HeaderTryFrom;
/// #
/// # fn main() {
/// #
/// # let me = Email::try_from("me@thisisme.mememe").unwrap().into();
/// # let an_additional_from = Email::try_from("notme@thisisntme.notmenotmenotme").unwrap().into();
/// # let some_one_else = Email::try_from("other@person.that_is").unwrap().into();
/// # let test_data = HashMap::<&'static str, String>::new();
/// let mut builder = MailSendDataBuilder::new();
/// builder
///     .sender(me)
///     .add_from(an_additional_from)
///     .add_to(some_one_else)
///     .subject("Una test")
///     .template("template_a1_b")
///     .data(test_data);
///
/// // build() consumes the builder so we can not chain
/// // it with the the other calls to the builder
/// let mail_send_data = builder.build().unwrap();
/// # }
/// ```
///
pub struct MailSendData<'a, TId: ?Sized + 'a, D>
    where TId: ToOwned, D: Serialize
{
    sender: Option<Mailbox>,
    from: MailboxList,
    to: MailboxList,
    subject: String,
    template_id: Cow<'a, TId>,
    data: D
}

impl<'a, TId: ?Sized + 'a, D> Debug for MailSendData<'a, TId, D>
    where TId: ToOwned + Debug, <TId as ToOwned>::Owned: Debug, D: Serialize + Debug
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


impl<'a, TId: ?Sized + 'a, D> MailSendData<'a, TId, D>
    where TId: ToOwned, D: Serialize
{
    /// create a simple MailSendData with a sing From and a single To Mailbox
    pub fn simple_new<I>(
        from: Mailbox, to: Mailbox,
        subject: I,
        template_id: Cow<'a, TId>, data: D
    ) -> Self
        where I: Into<String>
    {
        MailSendData {
            sender: None,
            from: MailboxList(vec1![from]),
            to: MailboxList(vec1![to]),
            subject: subject.into(),
            template_id, data
        }
    }

    /// returns a reference to a explicity set sender or else the first (and only) from mailbox
    pub fn sender(&self) -> &Mailbox {
        self.sender.as_ref().unwrap_or_else(|| self.from.first())
    }


    pub fn _from(&self) -> &MailboxList {
        &self.from
    }

    /// Allows mutating from Mailboxes
    ///
    /// this does only expose a &mut Slice of Mailboxes, instead of a &mut MailboxList
    /// to make sure that no from mailbox can be added as sender might be empty
    pub fn _from_mut(&mut self) -> &mut [Mailbox] {
        &mut self.from
    }

    //TODO add set_sender method
    //TODO add try_add_from method failing if sender is None
    //TODO maybe add a try_set_from(MailboxList) too

    pub fn _to(&self) -> &MailboxList {
        &self.to
    }

    pub fn _to_mut(&mut self) -> &mut MailboxList {
        &mut self.to
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn subject_mut(&mut self) -> &mut String {
        &mut self.subject
    }

    pub fn template(&self) -> &TId {
        &self.template_id
    }

    pub fn template_mut(&mut self) -> &mut Cow<'a, TId> {
        &mut self.template_id
    }

    pub fn data(&self) -> &D {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut D {
        &mut self.data
    }

    pub(crate) fn destruct(self) ->
        (Option<Mailbox>, MailboxList, MailboxList, String, Cow<'a, TId>, D)
    {
        //use let destruction to make it more refactoring resistend
        let MailSendData { sender, from, to, subject, template_id, data } = self;
        (sender, from, to, subject, template_id, data)
    }

    pub fn auto_gen_display_names<NC>(&mut self, name_composer: NC) -> Result<(), Error>
        where NC: NameComposer<D>
    {
        let data = &mut self.data;
        {
            let mut from_auto_gen = |email: &Email| {
                match name_composer.compose_from_name(email, data)? {
                    Some(name) => Ok(Some(Phrase::try_from(name)?)),
                    None => Ok(None),
                }
            };

            if let Some(sender) = self.sender.as_mut() {
                sender.auto_gen_name(&mut from_auto_gen)?;
            }

            for elem in self.from.iter_mut() {
                elem.auto_gen_name(&mut from_auto_gen)?;
            }
        }

        for elem in self.to.iter_mut() {
            elem.auto_gen_name(|email| {
                match name_composer.compose_to_name(email, data)? {
                    Some(name) => Ok(Some(Phrase::try_from(name)?)),
                    None => Ok(None),
                }
            })?;
        }

        Ok(())
    }
}

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
    fn compose_from_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error>;

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
    fn compose_to_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error>;

}

impl<D, T> NameComposer<D> for Arc<T>
    where T: NameComposer<D>, D: Serialize
{
    fn compose_from_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error> {
        self.deref().compose_from_name(email, data)
    }
    fn compose_to_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error> {
        self.deref().compose_to_name(email, data)
    }
}

impl<D, T> NameComposer<D> for Box<T>
    where T: NameComposer<D>, D: Serialize
{
    fn compose_from_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error> {
        self.deref().compose_from_name(email, data)
    }
    fn compose_to_name( &self, email: &Email, data: &mut D ) -> Result<Option<String>, Error> {
        self.deref().compose_to_name(email, data)
    }
}


/// Builder to create `MailSendData`
pub struct MailSendDataBuilder<'a, TId: ?Sized + 'a, D>
    where TId: ToOwned, D: Serialize
{
    sender: Option<Mailbox>,
    from: Vec<Mailbox>,
    to: Vec<Mailbox>,
    subject: Option<String>,
    template_id: Option<Cow<'a, TId>>,
    data: Option<D>
}

impl<'a, TId: ?Sized + 'a, D> Debug for MailSendDataBuilder<'a, TId, D>
    where TId: ToOwned + Debug, <TId as ToOwned>::Owned: Debug, D: Serialize + Debug
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
    where TId: ToOwned, D: Serialize
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, TId: ?Sized + 'a, D> MailSendDataBuilder<'a, TId, D>
    where TId: ToOwned, D: Serialize
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
    pub fn build(self) -> Result<MailSendData<'a, TId, D>, Self> {
        if self.from.is_empty() || self.to.is_empty()
            || self.subject.is_none() || self.template_id.is_none()
            || self.data.is_none()
        {
            return Err(self)
        }
        if self.from.len() > 1 && self.sender.is_none() {
            return Err(self)
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
}