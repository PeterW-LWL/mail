use std::path::PathBuf;
use std::fmt;

use futures::future::BoxFuture;

use error::Error;

use super::mime::SinglepartMime;
use types::buffer::FileBuffer;


pub enum Resource {
    File {
        //FIXME make it optional and use mime sniffing
        // sniff with magical number and file ending
        mime: SinglepartMime,
        path: PathBuf,
        alternate_name: Option<String>
    },
    FileBuffer( FileBuffer ),
    Future( BoxFuture<FileBuffer, Error> )
}

impl fmt::Debug for Resource {
    fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
        use self::Resource::*;
        match *self {
            File { ref mime, ref path, ref alternate_name } => {
                fter.debug_struct("File")
                    .field( "mime", mime )
                    .field( "path", path )
                    .field( "alternate_name", alternate_name)
                    .finish()
            },
            FileBuffer( ref buf ) => {
                fter.debug_tuple("FileBuffer").field( buf ).finish()
            },
            Future( .. ) => {
                write!( fter, "Future( .. )")
            }
        }
    }
}