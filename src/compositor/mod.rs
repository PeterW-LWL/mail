use std::borrow::ToOwned;

use serde::Serialize;

use error::Result;
use context::Context;
use template::TemplateEngine;
use mail::Mail;
use mail::headers::components::{MailboxList, Mailbox};

//TODO make sure Box/Arc auto wrapping is impl for all parts
use self::_impl::InnerCompositionBaseExt;

mod mail_send_data;
mod _impl;
mod composition_base_impl;

pub use self::mail_send_data::{MailSendData, MailSendDataBuilder, NameComposer};
pub use self::composition_base_impl::{SimpleCompositionBase, SharedCompositionBase};


/// Types implementing this do Expose both a template engine impl. and a context.
///
/// The `compose_mail` default impl does link into this crates internal impl. for
/// composing mails and should not be overriden, the main reason it is there, is
/// so that the internal Extensions trait used for implementing mail composition
/// does not need to be exposed.
///
pub trait CompositionBase {

    type Context: Context;
    type TemplateEngine: TemplateEngine<Self::Context>;

    /// composes a mail based on the given MailSendData
    fn compose_mail<D>(
        &self,
        send_data: MailSendData<
            <Self::TemplateEngine as TemplateEngine<Self::Context>>::TemplateId, D>
    ) -> Result<(Mail, EnvelopData)>
        where D: Serialize
    {
        InnerCompositionBaseExt::_compose_mail(self, send_data)
    }

    fn template_engine(&self) -> &Self::TemplateEngine;
    fn context(&self) -> &Self::Context;
}


//NOTE: this might get more complex at some point, wrt. e.g. cc, bcc, resent etc.
pub struct EnvelopData {
    sender: Mailbox,
    to: MailboxList
    //cc: MailboxList, //add if added to MailSendData
    //bcc: MailboxList, //add if added to MailSendData
}

impl EnvelopData {

    pub fn new(sender: Mailbox, to: MailboxList) -> Self {
        EnvelopData {
            sender, to
        }
    }

    pub fn sender(&self) -> &Mailbox {
        &self.sender
    }

    pub fn _to(&self) -> &MailboxList {
        &self.to
    }
}

impl<'a, T: ?Sized, D> From<&'a MailSendData<'a, T, D>> for EnvelopData
    where T: ToOwned, D: Serialize
{
    fn from(msd: &'a MailSendData<'a, T, D>) -> Self {
        let sender = msd.sender().clone();
        let to = msd._to().clone();
        EnvelopData::new(sender, to)
    }
}