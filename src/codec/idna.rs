use error::*;

use idna;

use ascii::AsciiString;


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