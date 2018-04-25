use futures::{Future, IntoFuture};

use mail::utils::SendBoxFuture;
use mail::context::{BuilderContext, Source, LoadResourceFuture};
use headers::components::ContentID;


// TODO extend interface to allow some per mail specifics e.g. gen content id
//      like `format!(_prefix_{}_{}, mail_cid_count, random)`
//NOTE: Sized is just as long as Serialize is used for data
pub trait Context: BuilderContext + Send + Sync {
    fn new_content_id(&self) -> ContentID;
}

pub trait ContentIdGenComponent {
    fn new_content_id(&self) -> ContentID;
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
    fn new_content_id( &self ) -> ContentID {
        self.id_gen.new_content_id()
    }
}

impl<T> ContentIdGenComponent for T
    where T: Context + Send + Sync + Clone + 'static
{
    fn new_content_id( &self ) -> ContentID {
        <Self as Context>::new_content_id(self)
    }
}