use std::sync::Arc;
use std::fmt;
use std::path::Path;

use futures::{ Future, BoxFuture };
use futures_cpupool::{ CpuPool, Builder };

use error::*;
use mail::{ FileLoader, RunElsewhere, CompositeBuilderContext };
use mail_composition::ContentIdGen;
use components::MessageID;

use super::VFSFileLoader;
use super::RandomContentId;



#[derive(Debug, Clone)]
pub struct SimpleContext( Arc<SimpleContextInner> );

struct SimpleContextInner {
    builder_context: CompositeBuilderContext<VFSFileLoader, CpuPool>,
    content_id_gen: RandomContentId
}

impl SimpleContext {

    pub fn new( content_id_postfix: String ) -> Self {
        Self::with_vfs( content_id_postfix, VFSFileLoader::new() )
    }

    pub fn with_cpupool(
        content_id_postfix: String,
        cpupool: Builder
    ) -> Self {
        Self::with_vfs_and_cpupool( content_id_postfix, VFSFileLoader::new(), cpupool )
    }

    pub fn with_vfs( content_id_postfix: String, vfs: VFSFileLoader ) -> Self {
        Self::with_vfs_and_cpupool( content_id_postfix, vfs, Builder::new() )
    }


    pub fn with_vfs_and_cpupool(
        content_id_postfix: String,
        vfs: VFSFileLoader,
        mut cpupool: Builder
    ) -> Self {
        SimpleContext( Arc::new(
            SimpleContextInner {
                builder_context: CompositeBuilderContext::new(
                    vfs,
                    cpupool.create()
                ),
                content_id_gen: RandomContentId::new( content_id_postfix )
            }
        ))
    }
}

impl fmt::Debug for SimpleContextInner {
    fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
        fter.debug_struct( "SimpleContext" )
            .field( "content_id_gen", &self.content_id_gen )
            .field( "file_loader", &self.builder_context.file_loader )
            .field( "elsewher", &"CpuPool { .. }" )
            .finish()
    }
}



impl FileLoader for SimpleContext {
    type FileFuture = <VFSFileLoader as FileLoader>::FileFuture;
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        self.0.builder_context.load_file( path )
    }
}

impl RunElsewhere for SimpleContext {
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        self.0.builder_context.execute( fut )
    }
}

impl ContentIdGen for SimpleContext {
    fn new_content_id(&self) -> Result<MessageID> {
        self.0.content_id_gen.new_content_id()
    }
}