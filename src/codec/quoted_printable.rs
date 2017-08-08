//FUTURE_TODO: make it it's own crate, possible push to `quoted_printable`
use ascii::{ AsciiChar, AsciiStr, AsciiString };
use error::*;
use types::Vec1;

use grammar::encoded_word::{ MAX_ECW_LEN, ECW_SEP_OVERHEAD };
use super::traits::{ EncodedWordWriter, MailEncoder };

pub struct VecWriter {
    data: Vec1<AsciiString >,
    max_len: usize
}

impl VecWriter {
    pub fn new(max_len: usize) -> Self {
        let data = Vec1::new( AsciiString::new() );
        VecWriter { data, max_len }
    }

    pub fn data( &self ) -> &[AsciiString] {
        &*self.data
    }
}

impl EncodedWordWriter for VecWriter {
    fn write_char( &mut self, ch: AsciiChar ) {
        self.data.last_mut().push( ch );
    }

    fn write_ecw_seperator( &mut self ) {
        self.data.push( AsciiString::new() )
    }

    fn write_ecw_start( &mut self ) {}

    fn write_ecw_end( &mut self ) {}

    fn max_payload_len( &self ) -> usize {
        self.max_len
    }
}

pub struct WriterWrapper<'a, E:'a>{
    charset: &'a AsciiStr,
    encoder: &'a mut E
}

impl<'a, E> WriterWrapper<'a, E> where E: MailEncoder + 'a {
    pub fn new( charset: &'a AsciiStr, encoder: &'a mut E ) -> Self {
        WriterWrapper { charset, encoder }
    }
}

impl<'a, E> EncodedWordWriter for WriterWrapper<'a, E> where E: MailEncoder + 'a {

    fn write_char( &mut self, ch: AsciiChar ) {
        self.encoder.write_char( ch )
    }

    fn write_ecw_seperator( &mut self ) {
        self.encoder.write_fws();
    }

    fn write_ecw_start( &mut self ) {
        self.encoder.write_char( AsciiChar::Equal );
        self.encoder.write_char( AsciiChar::Question );
        self.encoder.write_str( self.charset );
        self.encoder.write_char( AsciiChar::Question );
        // this is (for now) just for quoted printable
        self.encoder.write_char( AsciiChar::Q );
        self.encoder.write_char( AsciiChar::Question );
    }

    fn write_ecw_end( &mut self ) {
        self.encoder.write_char( AsciiChar::Question );
        self.encoder.write_char( AsciiChar::Equal );
    }

    fn max_payload_len( &self ) -> usize {
        MAX_ECW_LEN - ECW_SEP_OVERHEAD - self.charset.len() - 1
    }
}



///
/// Quoted Printable encoding for MIME-Headers
///
/// Which means:
/// 1. there is a limit to the maximum number of characters
///    - the limit is 75 INCLUDING the `=?charset?encoding?...?=` overhead
///    - as such the line length limit of quoted printable can not be hit,
///      the quoted printable part is at most 67 chars long, e.g. for utf8
///      it is at most 64 chars
/// 2. has to be one token, so no ' ','\t' and neither soft nor hard newlines
/// 3. no '?' character
///
/// The input is a sequence of bytes split up in chunks where
/// a split in multipl encoded words can be done between any
/// two chunks but not in a chunk. Wrt. utf8 a chunk would
/// correspond to a character, e.g. `[65]` for `'a'` and
/// `[0xe2, 0x99, 0xa5]` for a `'♥'`.
///
/// Note that a chunk can with more than 21 byte is not guranteed to
/// work, and can trigger a panic.
///
/// As this has to be safe for usage in all header contexts, additional
/// to the chars required by the standard (i.e. '=') following chars are ALWAYS
/// quoted' ', '\t', '?', '(', ')'. Also '\n','\r' see the note below for more
/// details.
///
/// # Error:
///
/// a error is returned if a single encoded chunk can not be written as one
/// because of the length limitation AFTER a new encoded word was started.
///
/// # Panics:
///
/// 1. if the encoded size of a chunk is more than 16 byte, which can
///    happen if a chunk has more than 5 bytes. For comparison utf8 has
///    at most chunks with 4 bytes leading to at most 12 byte buffer usage.
///
/// 2. if max size if >76 as no new line handling is implemented and
///    the max size for the use case can be at most 67 chars
///
/// # Note:
///   as it has to be a token no new line characters can appear in the output,
///   BUT q-encoding also forbids the encoding of CRLF line breaks in TEXT!
///   bodies, which is mean to not mess up with the limitations to the line
///   length, but they are allowed to appear in non TEXT data, but this
///   function should, but might not be limited to be used with text data,
///   which should but might not be limited to data not containing any new
///   line character. For now any appearance of '\r' or '\n' will be encoded
///   like any other "special" byte, for the future a context might be needed.
///   (Especially as encoded words can contain non-ascii text in which '\r','\n'
///   might be encoded with completely different bytes, but when the RFC speaks of
///   '\r','\n' it normally means the bytes 10/13 independent of the character set,
///   or if they appear in a image, zip-archiev etc. )
pub fn header_encode<'a, I, O>(input: I, out: &mut O, max_size: usize) -> Result<()>
    where I: Iterator<Item=&'a [u8]>, O: EncodedWordWriter
{

    assert!( max_size <= 76 );
    let mut remaining = max_size;
    let mut buf = [33; 16];

    for chunk in input {
        let mut buf_idx = 0;

        for byte in chunk {
            let byte = *byte;
            match byte {
                // this is the way to go as long as we don't want to behave differently for
                // different context, the COMMENT context allows more chars, and the
                // TEXT context even more
                b'!' | b'*' |
                b'+' | b'-' |
                b'/' | b'_' |
                b'0'...b'9' |
                b'A'...b'Z' |
                b'a'...b'z'  => {
                    buf[buf_idx] = byte;
                    buf_idx += 1;
                },
                _otherwise => {
                    buf[buf_idx] = b'=';
                    buf[buf_idx+1] = lower_nibble_to_hex( byte >> 4 ) as u8;
                    buf[buf_idx+2] = lower_nibble_to_hex( byte ) as u8;
                    buf_idx += 3;
                }
            }
        }
        if buf_idx > remaining {
            remaining = out.start_new_encoded_word();
        }
        if buf_idx > remaining {
            bail!( "single character longer then max length ({:?}) of encoded word", remaining );
        }
        for idx in 0..buf_idx {
            out.write_char( unsafe { AsciiChar::from_unchecked( buf[idx] ) } )
        }
        remaining -= buf_idx;
    }
    Ok( () )

}


macro_rules! ascii_table {
    ($($ch:ident)*) => {{
        &[ $(AsciiChar::$ch),* ]
    }}
}

fn lower_nibble_to_hex( half_byte: u8 ) -> AsciiChar {
    let chars = ascii_table! {
        _0 _1 _2 _3 _4 _5 _6 _7 _8 _9
        A B C D E F
    };

    chars[ (half_byte & 0x0F) as usize ]
}




#[cfg(test)]
mod test {
    use super::*;


    #[test]
    fn to_hex() {
        let data = &[
            (AsciiChar::_0, 0b11110000),
            (AsciiChar::_0, 0b0 ),
            (AsciiChar::_7, 0b0111),
            (AsciiChar::_7, 0b10111),
            (AsciiChar::F,  0b1111)
        ];
        for &(char, byte) in data {
            assert_eq!( char, lower_nibble_to_hex( byte) );
        }
    }

    macro_rules! test {
        ($name:ident, data $data:expr => [$($item:expr),*]) => {
            #[test]
            fn $name() {
                let test_data = $data;
                let mut out = VecWriter::new( 67 );
                let iter = test_data.char_indices().map( |(idx, ch)| {
                    &test_data.as_bytes()[idx..idx+ch.len_utf8()]
                });
                header_encode( iter, &mut out, 67 ).unwrap();

                let expected = &[
                    $($item),*
                ];
                let iter = expected.iter()
                    .zip( out.data().iter().map(|x|x.as_str()) )
                    .enumerate();

                for ( idx, (expected, got) ) in iter {
                    if  *expected != got {
                        panic!( " item nr {}: {:?} != {:?} ", idx, expected, got );
                    }
                }

                let e_len = expected.len();
                let g_len = out.data().len();
                if e_len > g_len {
                    panic!( "expected following additional items: {:?}", &expected[g_len..e_len])
                }
                if e_len < g_len {
                    panic!( "got following additional items: {:?}", &out.data()[e_len..g_len])
                }
            }
        };
    }

    test! { can_be_used_in_comments,
        data "()\"" => [
            "=28=29=22"
        ]
    }

    test! { can_be_used_in_phrase,
        data "{}~@#$%^&*()=|\\[]';:." => [
            "=7B=7D=7E=40=23=24=25=5E=26*=28=29=3D=7C=5C=5B=5D=27=3B=3A=2E"
        ]
    }

    test! { bad_chars_in_all_contexts,
        data "?= \t\r\n" => [
            "=3F=3D=20=09=0D=0A"
        ]
    }

    test!{ encode_ascii,
        data  "abcdefghijklmnopqrstuvwxyz \t?=0123456789!@#$%^&*()_+-" => [
             "abcdefghijklmnopqrstuvwxyz=20=09=3F=3D0123456789!=40=23=24=25=5E=26",
             "*=28=29_+-"
        ]
    }

    test! { how_it_handales_newlines,
        data "\r\n" => [
            "=0D=0A"
        ]
    }


    test! { split_into_multiple_ecws,
        data "0123456789012345678901234567890123456789012345678901234567891234567newline" => [
            "0123456789012345678901234567890123456789012345678901234567891234567",
            "newline"
        ]
    }

    test!{ bigger_chunks,
        data "ランダムテキスト ראַנדאָם טעקסט" => [
            //ランダムテキス
            "=E3=83=A9=E3=83=B3=E3=83=80=E3=83=A0=E3=83=86=E3=82=AD=E3=82=B9",
            //ト ראַנדאָם
            "=E3=83=88=20=D7=A8=D7=90=D6=B7=D7=A0=D7=93=D7=90=D6=B8=D7=9D=20",
            //ראַנדאָם
            "=D7=98=D7=A2=D7=A7=D7=A1=D7=98"
        ]
    }

}