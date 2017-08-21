use std::sync::Arc;
use std::path::Path;
use std::ops::Deref;


use futures::{ Future, IntoFuture };
use futures::future::{ self,  BoxFuture };

use error::*;


pub trait FileLoader {
    type FileFuture: Future<Item=Vec<u8>, Error=Error> + Send + 'static;
    /// load file specified by path, wile it returns
    /// a future it is not required to load the file
    /// in the background, but it is required to load
    /// it in context of polling this futures, e.g.
    /// by using `futures::lazy`
    fn load_file( &self, path: &Path ) -> Self::FileFuture;
}

impl<F: FileLoader> FileLoader for Arc<F> {
    type FileFuture = F::FileFuture;
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        self.deref().load_file( path )
    }
}

pub trait RunElsewhere {
    /// executes the futures `fut` "elswhere" e.g. in a cpu pool
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static;

    fn execute_fn<FN, I>( &self, fut: FN ) -> BoxFuture<I::Item, I::Error>
        where FN: FnOnce() -> I + Send + 'static,
              I: IntoFuture + 'static,
              I::Future: Send + 'static,
              I::Item: Send + 'static,
              I::Error: Send + 'static
    {
        self.execute( future::lazy( fut ) )
    }
}

impl<I: RunElsewhere> RunElsewhere for Arc<I> {
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.deref().execute( fut )
    }
}

pub trait BuilderContext: FileLoader + RunElsewhere + Clone + Send + Sync + 'static {}
impl<T> BuilderContext for T where T: FileLoader + RunElsewhere + Clone + Send + Sync + 'static {}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub struct CompositeBuilderContext<FL, EW> {
    pub file_loader: FL,
    pub elsewhere: EW
}

impl<FL, EW> CompositeBuilderContext<FL, EW>
    where FL: FileLoader,
          EW: RunElsewhere
{
    pub fn new( file_loader: FL, elsewhere: EW ) -> Self {
        CompositeBuilderContext { file_loader, elsewhere }
    }
}

impl<FL: FileLoader, EW>  FileLoader for CompositeBuilderContext<FL, EW> {
    type FileFuture = FL::FileFuture;
    /// load file specified by path, wile it returns
    /// a future it is not required to load the file
    /// in the background, as such you should not relay
    /// on it beeing non-blocking, it might just load
    /// the file in place and return futures::ok
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        self.file_loader.load_file( path )
    }
}

impl<FL, EW: RunElsewhere> RunElsewhere for CompositeBuilderContext<FL, EW> {
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.elsewhere.execute( fut )
    }
}
