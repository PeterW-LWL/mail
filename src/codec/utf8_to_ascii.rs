use error::*;

use idna;

use ascii::AsciiString;
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


//TODO(refactor): make an idna module which wraps idna
// provides a domain_to_ascii function returning
// a AsciiString and a Error type wrapper implementing
// std Error, so that error_chain foreign_links can be
// used

/// uses puny code on given domain to return a ascii representation
///
/// # Implementation Detail
/// this function uses `idna::domain_to_ascii`, see the
/// `idna` crates documentation fore more details on how
/// exactly all edcases are handled
///
/// # Note
/// that this function does not validat the domain, e.g.
/// if you puny code the domain `"this seems\0so;wrong"` it
/// will return `Ok("this seems\0so;wrong")`
///
pub fn puny_code_domain( domain: &str ) -> Result<AsciiString> {
    match idna::domain_to_ascii( domain ) {
        Ok( asciified ) => {
            //SAFE: well we converted it to ascii, so it's ascii
            Ok( unsafe { AsciiString::from_ascii_unchecked(asciified) } )
        },
        Err( err ) => {
            //FIXME(UPSTREAM): uts46::Errors does not implement Error... ;=(
            bail!(ErrorKind::PunyCodeingDomainFailed(err));
        }
    }
}


#[cfg(test)]
mod test {
    use idna;
    use super::puny_code_domain;

    #[test]
    fn idna_does_not_validate() {
        let domain = "this seems\0so;wrong";
        assert_eq!(
            domain.to_owned(),
            assert_ok!( idna::domain_to_ascii(domain) )
        );
    }

    #[test]
    fn nop_puny_code() {
        let domain = "is_ascii.notadomain";

        let encoded = assert_ok!( puny_code_domain( domain ) );
        assert_eq!(
            "is_ascii.notadomain",
            &*encoded
        );
    }
    #[test]
    fn puny_code_ascii_mail() {
        let domain = "nöt_ascii.ü";
        let encoded = assert_ok!( puny_code_domain(domain) );
        assert_eq!(
            "xn--nt_ascii-n4a.xn--tda",
            &*encoded
        );
    }
}