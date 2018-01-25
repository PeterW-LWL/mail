use std::ops::Deref;

use rand;
use rand::Rng;

use mheaders::components::MediaType;

use core::error::{ErrorKind, Result, ResultExt};
use core::HeaderTryInto;

use utils::is_multipart_mime;

/// write a random sequence of chars valide for and boundary to the output buffer
///
/// Note that it might be required to quote the boundary.
/// The boundary (excluding quotations) will start with `"=_^"` which is neither
/// valid for base64 nor quoted-printable encoding.
///
/// The boundary will be picked from ascii `VCHAR`'s (us-ascii >= 33 and <= 126) but
/// following `VCHAR`'s are excluded `'"'`, `'-'` and `'\\'`.
pub fn create_random_boundary() -> String {
    const MULTIPART_BOUNDARY_LENGTH: usize = 30;
    static CHARS: &[char] = &[
        '!',      '#', '$', '%', '&', '\'', '(',
        ')', '*', '+', ',',      '.', '/', '0',
        '1', '2', '3', '4', '5', '6', '7', '8',
        '9', ':', ';', '<', '=', '>', '?', '@',
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H',
        'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P',
        'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X',
        'Y', 'Z', '[',      ']', '^', '_', '`',
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h',
        'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p',
        'q', 'r', 's', 't', 'u', 'v', 'w', 'x',
        'y', 'z', '{', '|', '}', '~'
    ];

    // we add =_^ to the boundary, as =_^ is neither valid in base64 nor quoted-printable
    let mut out = String::with_capacity(MULTIPART_BOUNDARY_LENGTH);
    out.push_str("=_^");
    let mut rng = rand::thread_rng();
    for _ in 3..MULTIPART_BOUNDARY_LENGTH {
        out.push( CHARS[ rng.gen_range( 0, CHARS.len() )] )
    }
    out
}


#[derive(Debug)]
pub struct SinglepartMime( MediaType );

impl SinglepartMime {
    pub fn new( mime: MediaType ) -> Result<Self> {
        if !is_multipart_mime( &mime ) {
            Ok( SinglepartMime( mime ) )
        } else {
            Err( ErrorKind::NotSinglepartMime( mime.into() ).into() )
        }
    }
}

impl HeaderTryInto<MediaType> for SinglepartMime {
    fn try_into(self) -> Result<MediaType> {
        Ok( self.0 )
    }
}

impl Into<MediaType> for SinglepartMime {
    fn into( self ) -> MediaType {
        self.0
    }
}

impl Deref for SinglepartMime {
    type Target = MediaType;

    fn deref( &self ) -> &MediaType {
        &self.0
    }
}

#[derive(Debug)]
pub struct MultipartMime( MediaType );

impl MultipartMime {

    pub fn new( mime: MediaType ) -> Result<Self> {
        if is_multipart_mime( &mime ) {
            Ok( MultipartMime( mime ) )
        }  else {
            Err( ErrorKind::NotMultipartMime( mime.into() ).into() )
        }

    }
}

impl HeaderTryInto<MediaType> for MultipartMime {
    fn try_into(self) -> Result<MediaType> {
        Ok( self.0 )
    }
}

impl Into<MediaType> for MultipartMime {
    fn into( self ) -> MediaType {
        self.0
    }
}

impl Deref for MultipartMime {
    type Target = MediaType;

    fn deref( &self ) -> &MediaType {
        &self.0
    }
}

pub fn gen_multipart_mime<A>( subtype: A ) -> Result<MultipartMime>
    where A: AsRef<str>
{
    let boundary = create_random_boundary();
    let media_type = MediaType::new_with_params("multipart", subtype.as_ref(), vec![
        ("boundary", &*boundary)
    ]).chain_err(|| ErrorKind::GeneratingMimeFailed)?;
    Ok(MultipartMime(media_type))
}

#[cfg(test)]
mod test {

    mod write_random_boundary_to {
        use super::super::*;

        #[test]
        fn boundary_is_not_quoted() {
            let out = create_random_boundary();
            assert!(!out.starts_with("\""));
            assert!(!out.ends_with("\""));
        }

        #[test]
        fn boundary_start_special() {
            let out = create_random_boundary();
            assert!(out.starts_with("=_^"));
        }

        #[test]
        fn boundary_has_a_resonable_length() {
            let out = create_random_boundary();
            assert!(out.len() > 22 && out.len() < 100);
        }

        #[test]
        fn boundary_does_not_contain_space_or_slach_or_quotes() {
            // while it could contain them it's recommended not to do it
            let out = create_random_boundary();

            for ch in out[1..out.len()-1].chars() {
                assert!(ch as u32 >= 33);
                assert!(ch as u32 <= 126);
                assert_ne!(ch, ' ');
                assert_ne!(ch, '\t');
                assert_ne!(ch, '\\');
                assert_ne!(ch, '"');
            }

        }
    }
}