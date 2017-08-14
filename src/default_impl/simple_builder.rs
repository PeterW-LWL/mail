use std::sync::Arc;
use std::default::Default;
use std::path::Path;
use std::fmt;

use futures::{ Future, BoxFuture };
use futures_cpupool::{ CpuPool, Builder };
use super::VFSFileLoader;

use mail::{ FileLoader, RunElsewhere, CompositeBuilderContext };

#[derive(Clone)]
pub struct SimpleBuilderContext( Arc< CompositeBuilderContext<VFSFileLoader, CpuPool>  > );


impl fmt::Debug for SimpleBuilderContext {
    fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
        fter.debug_struct( "SimpleBuilderContext" )
            .field( "file_loader", &self.0.file_loader )
            .field( "elsewher", &"CpuPool { .. }" )
            .finish()
    }
}

impl Default for SimpleBuilderContext {
    fn default() -> Self {
        SimpleBuilderContext::new()
    }
}

impl SimpleBuilderContext {

    pub fn new() -> Self {
        SimpleBuilderContext( Arc::new( CompositeBuilderContext::new(
            VFSFileLoader::new(),
            Builder::new().create()
        ) ) )
    }

}


impl FileLoader for SimpleBuilderContext {
    type FileFuture = <VFSFileLoader as FileLoader>::FileFuture;
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        self.0.load_file( path )
    }
}

impl RunElsewhere for SimpleBuilderContext {
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.0.execute( fut )
    }
}