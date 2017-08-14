use std::path::{ PathBuf, Path };
use std::fs::File;
use std::io::Read;

use futures::future;

use error::*;
use mail::FileLoader;



#[derive( Debug, Clone, PartialEq, Default )]
pub struct FSFileLoader {
    root: PathBuf
    //FEATURE_TODO(opt_only_subfolder): if check_if_sub_dir is true make sure /../ is not used to
    //  navigate above root
    //check_if_sub_dir: bool
}

impl FSFileLoader {

    /// create a new file system based FileLoader, which will  "just" standard _blocking_ IO
    /// to read a file from the file system into a buffer
    pub fn new<P: Into<PathBuf>>( root: P ) -> Self {
        FSFileLoader { root: root.into() }
    }

}


impl FileLoader for FSFileLoader {
    type FileFuture = future::FutureResult<Vec<u8>, Error>;

    // will block, but this is within the expectancy of the interface
    fn load_file( &self, path: &Path ) -> Self::FileFuture {
        let pbuf;
        let path = if path.is_absolute() {
            path
        } else {
            pbuf = self.root.join( path );
            &*pbuf
        };
        future::result( read_file( path ) )
    }
}

fn read_file( path: &Path ) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file  = File::open( path )?;
    file.read_to_end( &mut buf )?;
    Ok( buf )
}


#[cfg(test)]
mod test {
    use futures::Future;
    use std::env::current_dir;
    use super::*;

    #[test]
    fn load_file_from_fs() {
        let fl =  FSFileLoader::new( current_dir().unwrap() );
        let _ = assert_ok!( fl.load_file( Path::new( "./Cargo.toml" ) ).wait() );
    }

    #[test]
    fn bad_load_file_from_fs() {
        let fl =  FSFileLoader::new( current_dir().unwrap() );
        let res = fl.load_file( Path::new( "./src/this_is_definitly_not_a_file.notafile" ) )
            .wait();

        assert_eq!( false, res.is_ok() );
    }
}