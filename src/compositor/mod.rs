use serde::Serialize;

use mail::Mail;

use ::context::Context;
use ::template::TemplateEngine;
use ::error::CompositionError;
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
    ) -> Result<
        Mail,
        CompositionError<<Self::TemplateEngine as TemplateEngine<Self::Context>>::Error>
    >
        where D: Serialize
    {
        InnerCompositionBaseExt::_compose_mail(self, send_data)
    }

    fn template_engine(&self) -> &Self::TemplateEngine;
    fn context(&self) -> &Self::Context;
}