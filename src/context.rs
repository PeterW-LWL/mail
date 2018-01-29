use std::sync::Arc;
use std::ops::Deref;
use std::path::Path;
use std::borrow::Cow;

use futures::Future;
use mail::utils::SendBoxFuture;

use core::error::Result;
use mail::context::{ FileLoader, RunElsewhere, BuilderContext };
use headers::components::{ Mailbox, MailboxList,  MessageID };

//TODO rename
pub struct MailSendContext {
    pub sender: Mailbox,
    pub other_from: Vec<Mailbox>,
    pub to: MailboxList,
    pub subject: String
}

impl MailSendContext {

    pub fn new(from: Mailbox, to: Mailbox, subject: String) -> Self {
        MailSendContext {
            sender: from,
            other_from: Vec::new(),
            to: MailboxList(vec1![ to ]),
            subject,
        }
    }
}

pub trait ContentIdGen {
    fn new_content_id( &self ) -> Result<MessageID>;
}

pub trait Context: BuilderContext + ContentIdGen + Send + Sync {}

impl<T> Context for T where T: BuilderContext + ContentIdGen + Send + Sync {}

impl<T: ContentIdGen> ContentIdGen for Arc<T> {
    fn new_content_id( &self ) -> Result<MessageID> {
        self.deref().new_content_id()
    }
}

pub struct ComposedContext<CIG, BC> {
    id_gen: CIG,
    builder_context: BC
}

impl<CIG, BC: BuilderContext> FileLoader for ComposedContext<CIG, BC> {
    type FileFuture = <BC as FileLoader>::FileFuture;
    fn load_file( &self, path: Cow<'static, Path> ) -> Self::FileFuture {
        self.builder_context.load_file( path )
    }
}

impl<CIG, BC: BuilderContext> RunElsewhere for ComposedContext<CIG, BC> {
    fn execute<F>( &self, fut: F) -> SendBoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.builder_context.execute( fut )
    }
}


impl<CIG: ContentIdGen, BC> ContentIdGen for ComposedContext<CIG, BC> {

    fn new_content_id( &self ) -> Result<MessageID> {
        self.id_gen.new_content_id()
    }
}



