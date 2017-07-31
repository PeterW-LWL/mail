use std::sync::Arc;
use std::ops::Deref;

use mail::BuilderContext;
use components::{ Mailbox,  MessageID };

//TODO replace with types::ContentId
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct ContentId( String );


pub trait Context: BuilderContext+Send+Sync {
    fn new_content_id( &self ) -> MessageID;
}

impl<T: Context> Context for Arc<T> {
    fn new_content_id( &self ) -> MessageID {
        self.deref().new_content_id()
    }
}


pub struct MailSendContext {
    pub from: Mailbox,
    pub to: Mailbox,
    pub subject: String
}
