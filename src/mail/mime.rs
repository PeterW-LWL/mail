use std::ops::Deref;

use rand;
use rand::Rng;
use mime::Mime;

use error::*;
use utils::{ is_multipart_mime, HeaderTryInto };


/// write a random sequence of chars valide for and boundary to the output buffer
///
/// The boundary will be quoted, i.e. start and end with `'"'`.
/// The boundary (excluding quotations) will start with `"=_^"` which is neither
/// valid for base64 nore quoted-printable encoding.
///
/// The boundary will be picked from ascii `VCHAR`'s (us-ascii >= 33 and <= 126) but
/// following `VCHAR`'s are excluded `'"'`, `'-'` and `'\\'`.
pub fn write_random_boundary_to(out: &mut String) {
    //TODO(CONFIG): make this configurable
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

    // we add =_^ to the boundary, as =_^ is neither valide in base64 nor quoted-printable
    out.push_str("\"=_^");
    let mut rng = rand::thread_rng();
    for _ in 0..MULTIPART_BOUNDARY_LENGTH {
        out.push( CHARS[ rng.gen_range( 0, CHARS.len() )] )
    }
    out.push('"');
}


#[derive(Debug)]
pub struct SinglepartMime( Mime );

impl SinglepartMime {
    pub fn new( mime: Mime ) -> Result<Self> {
        if !is_multipart_mime( &mime ) {
            Ok( SinglepartMime( mime ) )
        } else {
            Err( ErrorKind::NotSinglepartMime( mime ).into() )
        }
    }
}

impl HeaderTryInto<Mime> for SinglepartMime {
    fn try_into(self) -> Result<Mime> {
        Ok( self.0 )
    }
}

impl Into<Mime> for SinglepartMime {
    fn into( self ) -> Mime {
        self.0
    }
}

impl Deref for SinglepartMime {
    type Target = Mime;

    fn deref( &self ) -> &Mime {
        &self.0
    }
}

#[derive(Debug)]
pub struct MultipartMime( Mime );

impl MultipartMime {

    pub fn new( mime: Mime ) -> Result<Self> {
        if is_multipart_mime( &mime ) {
            Ok( MultipartMime( mime ) )
        }  else {
            Err( ErrorKind::NotMultipartMime( mime ).into() )
        }

    }
}

impl HeaderTryInto<Mime> for MultipartMime {
    fn try_into(self) -> Result<Mime> {
        Ok( self.0 )
    }
}

impl Into<Mime> for MultipartMime {
    fn into( self ) -> Mime {
        self.0
    }
}

impl Deref for MultipartMime {
    type Target = Mime;

    fn deref( &self ) -> &Mime {
        &self.0
    }
}



#[cfg(test)]
mod test {

    mod write_random_boundary_to {
        use super::super::*;

        #[test]
        fn boundary_is_quoted() {
            let mut out = String::new();
            write_random_boundary_to(&mut out);
            assert!(out.starts_with("\""));
            assert!(out.ends_with("\""));
        }

        #[test]
        fn boundary_start_special() {
            let mut out = String::new();
            write_random_boundary_to(&mut out);
            assert!(out.starts_with("\"=_^"));
        }

        #[test]
        fn boundary_has_a_resonable_length() {
            let mut out = String::new();
            write_random_boundary_to(&mut out);
            assert!(out.len() > 22 && out.len() < 100);
        }

        #[test]
        fn boundary_does_not_contain_space_or_slach_or_quotes() {
            // while it could contain them it's recommended not to do it
            let mut out = String::new();
            write_random_boundary_to(&mut out);

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