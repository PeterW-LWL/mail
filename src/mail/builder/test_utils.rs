use std::io;
use std::path::{ PathBuf, Path };
use std::collections::HashMap;

use futures::{ self, Future };
use futures::future::BoxFuture;

use error::*;
use super::{ FileLoader, RunElsewhere, CompositeBuilderContext };

pub type TestBuilderContext = CompositeBuilderContext<TestFileLoader, TestElsewhere>;

#[derive( Debug, Clone, PartialEq, Default )]
pub struct TestFileLoader {
    files: HashMap<PathBuf, Vec<u8>>
}

impl TestFileLoader {

    pub fn new() -> Self {
        TestFileLoader { files: HashMap::new() }
    }

    /// registers a file to the file_loader part of test context,
    /// if a file under the given path already did exist, its
    /// content is overwritten and the old content is returned
    pub fn register_file( &mut self,  path: PathBuf, content: Vec<u8> ) -> Option< Vec<u8> > {
        self.files.insert( path , content )
    }
}

impl FileLoader for TestFileLoader {
    type FileFuture = futures::future::FutureResult<Vec<u8>, Error>;

    /// load file specified by path, wile it returns
    /// a future it is not required to load the file
    /// in the background, as such you should not relay
    /// on it beeing non-blocking, it might just load
    /// the file in place and return futures::ok
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        let result = if let Some( file ) = self.files.get( path ) {
            Ok( file.clone() )
        } else {
            let msg = path.to_string_lossy().into_owned();
            let err: Error = io::Error::new( io::ErrorKind::NotFound, msg ).into();
            Err( err )
        };
        futures::future::result( result )
    }
}

#[derive( Debug, Clone, Hash, Eq, PartialEq, Default )]
pub struct TestElsewhere;

impl RunElsewhere for TestElsewhere {
    /// executes the futures `fut` "elswhere" e.g. in a cpu pool
    fn execute<F>( &self, fut: F) -> BoxFuture<F::Item, F::Error>
        where F: Future + Send + 'static,
              F::Item: Send+'static,
              F::Error: Send+'static
    {
        //FIXME cpupool? for now it doesn't run elsewhere
        fut.boxed()
    }
}

