use std::path::{ PathBuf, Path };
use std::collections::HashMap;
use std::io;

use futures::future;

use error::*;
use mail::FileLoader;

#[derive( Debug, Clone, PartialEq, Default )]
pub struct VFSFileLoader {
    files: HashMap<PathBuf, Vec<u8>>
}

impl VFSFileLoader {

    pub fn new() -> Self {
        VFSFileLoader { files: HashMap::new() }
    }

    /// registers a file to the file_loader part of test context,
    /// if a file under the given path already did exist, its
    /// content is overwritten and the old content is returned
    pub fn register_file<IP, IV>( &mut self,  path: IP, content: IV ) -> Result< Option< Vec<u8> > >
        where IP: Into<PathBuf>, IV: Into<Vec<u8>>
    {
        let path = path.into();
        let path = if path.is_relative() {
            Path::new( "/" ).join( path )
        } else { path };

        Ok( self.files.insert( path , content.into() ) )
    }
}


impl FileLoader for VFSFileLoader {
    type FileFuture = future::FutureResult<Vec<u8>, Error>;

    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        let pbuf;
        let path = if path.is_relative() {
            pbuf = Path::new( "/" ).join( path );
            &*pbuf
        } else { path };

        let result = if let Some( file ) = self.files.get( path ) {
            Ok( file.clone() )
        } else {
            let msg = path.to_string_lossy().into_owned();
            let err: Error = io::Error::new( io::ErrorKind::NotFound, msg ).into();
            Err( err )
        };
        future::result( result )
    }
}



#[cfg(test)]
mod test {
    use futures::Future;
    //use mail::FileLoader;
    use super::*;

    #[test]
    fn ok_if_registered() {
        let mut fl = VFSFileLoader::new();
        fl.register_file( "/my/resource.png", b"abcde".to_vec() ).unwrap();
        let res = fl.load_file( &*PathBuf::from( "/my/resource.png" ) ).wait();
        let buf = assert_ok!( res );
        assert_eq!(
            b"abcde".to_vec(),
            buf
        );
    }

    #[test]
    fn err_if_not_registered() {
        let mut fl = VFSFileLoader::new();
        fl.register_file( "/my/resource.png", b"abcde".to_vec() ).unwrap();
        let res = fl.load_file( &*PathBuf::from( "/not/my/resource.png" ) ).wait();
        assert_eq!( false, res.is_ok() );
    }

    #[test]
    fn handles_relative_paths_in_registry() {
        let mut fl = VFSFileLoader::new();
        fl.register_file( "./my/resource.png", b"abcde".to_vec() ).unwrap();
        let res = fl.load_file( &*PathBuf::from( "/my/resource.png" ) ).wait();
        let buf = assert_ok!( res );
        assert_eq!(
            b"abcde".to_vec(),
            buf
        );
    }

    #[test]
    fn handles_relative_paths_in_usage() {
        let mut fl = VFSFileLoader::new();
        fl.register_file( "/my/resource.png", b"abcde".to_vec() ).unwrap();
        let res = fl.load_file( &*PathBuf::from( "./my/resource.png" ) ).wait();
        let buf = assert_ok!( res );
        assert_eq!(
            b"abcde".to_vec(),
            buf
        );
    }

}