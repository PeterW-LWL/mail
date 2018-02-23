use std::borrow::Cow;

use serde::Serialize;

use futures::{Future, IntoFuture};
use mail::utils::SendBoxFuture;

use core::error::Result;
use mail::context::{BuilderContext, Source, LoadResourceFuture};
use headers::components::{ Mailbox, MailboxList,  MessageID };

//TODO maybe convert into a mail address bilder supporting
// From(1+), To(1+), Subject, Bcc, Cc
// e.g. .cc(Mailbox).bcc(Mailbox).from(Mailbox).to(Mailbox)
//      .sender(Mailbox) //adds Mailbox at pos 0 as sender overides previous sender if there is one
//      .to(Mailbox) //adds other_from
pub struct MailSendData<'a, TId: ?Sized + 'a, D>
    where TId: ToOwned, D: Serialize
{
    pub sender: Mailbox,
    pub other_from: Vec<Mailbox>,
    pub to: MailboxList,
    pub subject: String,
    pub template_id: Cow<'a, TId>,
    pub data: D
}

impl<'a, T: ?Sized + 'a, D> MailSendData<'a, T, D>
    where T: ToOwned, D: Serialize
{
    pub fn simple_new<I>(
        from: Mailbox, to: Mailbox,
        subject: I,
        template_id: Cow<'a, T>, data: D
    ) -> Self
        where I: Into<String>
    {
        MailSendData {
            sender: from,
            other_from: Vec::new(),
            to: MailboxList(vec1![to]),
            subject: subject.into(),
            template_id, data
        }
    }
}

// TODO extend interface to allow some per mail specifics e.g. gen content id
//      like `format!(_prefix_{}_{}, mail_cid_count, random)`
//NOTE: Sized is just as long as Serialize is used for data
pub trait Context: BuilderContext + Send + Sync {
    fn new_content_id( &self ) -> Result<MessageID>;
}

pub trait ContentIdGenComponent {
    fn new_content_id( &self ) -> Result<MessageID>;
}

#[derive(Debug, Clone)]
pub struct CompositeContext<I, B>
    where I: ContentIdGenComponent + Send + Sync + Clone + 'static,
          B: BuilderContext
{
    id_gen: I,
    builder_context: B
}

impl<I, B> CompositeContext<I, B>
    where I: ContentIdGenComponent + Send + Sync + Clone + 'static,
          B: BuilderContext
{
    pub fn new(id_gen: I, builder_context: B) -> Self {
        CompositeContext {
            id_gen, builder_context
        }
    }
}

impl<I, B> BuilderContext for CompositeContext<I, B>
    where I: ContentIdGenComponent + Send + Sync + Clone + 'static,
          B: BuilderContext
{
    fn load_resource( &self, source: &Source) -> LoadResourceFuture {
        self.builder_context.load_resource(source)
    }

    fn offload<F>(&self, future: F) -> SendBoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.builder_context.offload(future)
    }

    fn offload_fn<FN, IT>(&self, func: FN ) -> SendBoxFuture<IT::Item, IT::Error>
        where FN: FnOnce() -> IT + Send + 'static,
              IT: IntoFuture + 'static,
              IT::Future: Send + 'static,
              IT::Item: Send + 'static,
              IT::Error: Send + 'static
    {
        self.builder_context.offload_fn(func)
    }
}

impl<I, B> Context for CompositeContext<I, B>
    where I: ContentIdGenComponent + Send + Sync + Clone + 'static,
          B: BuilderContext
{
    fn new_content_id( &self ) -> Result<MessageID> {
        self.id_gen.new_content_id()
    }
}

impl<T> ContentIdGenComponent for T
    where T: Context + Send + Sync + Clone + 'static
{
    fn new_content_id( &self ) -> Result<MessageID> {
        <Self as Context>::new_content_id(self)
    }
}