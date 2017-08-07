//use std::ascii::AsciiExt;
use error::*;

use ascii::{ AsciiString, AsAsciiStr };
use codec::MailEncoder;
use quoted_printable;
use base64;
use grammar::encoded_word::EncodedWordContext;


macro_rules! base64_config {
    () => {
        // as we neither have const_fn constructors (currently) nor is
        // Config a POD (with public fields) a `const` wont work and
        // a lazy_static feels wrong ( I mean it's basically a
        // 4*8bit = 32bit )
        base64::Config::new(
            base64::CharacterSet::Standard,
            //padding
            true,
            //only relevant for decoding
            true,
            base64::LineWrap::NoWrap
        )
    }
}

pub fn base64_decode_for_encoded_word( input: &str ) -> Result<Vec<u8>> {
    Ok( base64::decode_config( input, base64_config!() )? )
}

pub fn base64_encoded_for_encoded_word( input: &str, _ctx: EncodedWordContext ) -> AsciiString {
    //FIXME ok for body but does not comply with header restrictions
    let res = base64::encode_config( input, base64_config!() );
    let asciied = unsafe { AsciiString::from_ascii_unchecked( res ) };
    asciied
}

pub fn q_decode_for_encoded_word( input: &str ) -> Result<Vec<u8>> {
    Ok( quoted_printable::decode( input.as_bytes(), quoted_printable::ParseMode::Strict )? )
}

//FEATURE_TODO(char_sing): instead of returning a AsciiString, pass in a Sink (
// which e.g. forwards to the Encoder)
pub fn q_encode_for_encoded_word( input: &str, _ctx: EncodedWordContext ) -> AsciiString {

    //TODO I suspect the `quoted_printable` crate is not
    // completely correct wrt. to some aspects, have to
    // check this
    //FIXME does need the current line length and wather or not it is a header
    //we have to encode ' ' in headers, q_encoded does NOT do this
    let raw = quoted_printable::encode( input.as_bytes() );//<- use ctx to limit characters
    let asciied = unsafe { AsciiString::from_ascii_unchecked( raw ) };
    asciied
}


pub fn puny_code_domain<E>( input: &str, encoder: &mut E )
    where E: MailEncoder
{
    //TODO there is a crate for it in the dependencies of the url crate
//    let mut out = String::new();
//    for ch in input.chars() {
//        if ch.is_ascii() {
//            out.push( ch )
//        } else {
//            //makar that there was non asci and which non asci
//        }
//    }
//
//    if has_non_ascii {
//        out.push( "-" );
//
//    }
    if let Ok( val ) = input.as_ascii_str() {
        encoder.write_str( val )
    } else {
        unimplemented!();
    }
}