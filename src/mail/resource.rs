use std::path::PathBuf;
use std::fmt;

use futures::future::BoxFuture;

use error::Error;

use super::mime::SinglepartMime;
use mime::TEXT_PLAIN;
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

impl Resource {

    /// creates a new Ressource with content type "text/plain" and an empty body
    pub fn empty_text() -> Self {
        Resource::FileBuffer( FileBuffer::new( TEXT_PLAIN, Vec::new() ) )
    }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_text_resource() {
        let er = Resource::empty_text();
        match er {
            Resource::FileBuffer( fb ) => {
                assert_eq!( &TEXT_PLAIN, fb.content_type() );
                assert_eq!( "".as_bytes(), &*fb );
            },
            other => panic!( "unexpected kind of empty text: {:?}", other )
        }
    }
}