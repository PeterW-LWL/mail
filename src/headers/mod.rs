use std::fmt;
use codec::MailEncoder;

use ascii::AsciiStr;
pub use ascii::{  AsciiStr as _AsciiStr };

use error::*;
use grammar::is_ftext;


#[macro_use]
mod default_header;
pub use self::default_header::*;

#[macro_use]
mod map;
pub use self::map::{HeaderMap, TypedMultiBodyIter, UntypedMultiBodyIter};


pub trait Header {
    const CAN_APPEAR_MULTIPLE_TIMES: bool;
    type Component;

    //FIXME turn this into a Accosiated constant once it is possible
    // requires at last `const fn` support in stable and the ascii
    // crate appear
    fn name() -> HeaderName;

    //NOTE: this is a circular dependency between Header/HeaderMap
    // but putting up e.g. a GeneraicHeaderMap trait/interface is
    // not worth the work at all
    /// Returns a function which is meant to be called with a reference
    /// to the final header map before encoding the headers. It is
    /// meant to be used do some of the contextual validations,
    /// like e.g. a `From` header might return a function which
    /// checks if the `From` header has multiple mailboxes and
    /// if so checks if there is a `Sender` header
    ///
    /// Calling a contextual validator with a header map not
    /// containing a header which it is meant to validate
    /// should not cause an error. Only if the header is
    /// there and the component is of the expected type
    /// and it is invalid in the context
    /// an error should be returned.
    fn get_contextual_validator<E>() -> Option<fn(&HeaderMap<E>) -> Result<()>>
        where E: MailEncoder
    {
        None
    }
}

/// all headers defined with `def_headers!` where
/// `CAN_APPEAR_MULTIPLE_TIMES` is `false` do implement
/// `SingularHeaderMarker` which is required to use
/// the `HeaderMap::get_single` functionality.
pub trait SingularHeaderMarker {}

/// a utility trait allowing us to use type hint structs
/// in `HeaderMap::{contains, get_untyped}`
pub trait HasHeaderName {
    fn get_name(&self) -> HeaderName;
}

impl HasHeaderName for HeaderName {
    fn get_name(&self) -> HeaderName {
        *self
    }
}

impl<H> HasHeaderName for H
    where H: Header
{
    fn get_name(&self) -> HeaderName {
        H::name()
    }
}

///
/// Note: Normally you will never have the need to create a HeaderName instance by
/// yourselve (except maybe for testing). At last as long as you use `def_header!`
/// for defining custom Headers, which is highly recommendet
///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct HeaderName {
    name: &'static AsciiStr
}

impl HeaderName {
    ///
    /// Be aware, that this libary only accepts header names with a letter case,
    /// that any first character of an alphanumeric part of a header name has to
    /// be uppercase and all other lowercase. E.g. `Message-Id` is accepted but
    /// `Message-ID` is rejected, even through both are _semantically_ the same.
    /// This frees us from doing eith case insensitive comparsion/hash wrt. hash map
    /// lookups, or converting all names to upper/lower case.
    ///
    pub fn new( name: &'static AsciiStr ) -> Result<Self> {
        HeaderName::validate_name( name )?;
        Ok( HeaderName { name } )
    }
    pub unsafe fn from_ascii_unchecked<B: ?Sized>( name: &'static B ) -> HeaderName
        where B: AsRef<[u8]>
    {
        HeaderName { name: AsciiStr::from_ascii_unchecked( name ) }
    }

    #[inline(always)]
    pub fn as_ascii_str( &self ) -> &'static AsciiStr {
        self.name
    }
    #[inline(always)]
    pub fn as_str( &self ) -> &'static str {
        self.name.as_str()
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "{}", self.as_str())
    }
}

impl PartialEq<str> for HeaderName {
    fn eq(&self, other: &str) -> bool {
        self.name.as_str() == other
    }
}

impl PartialEq<AsciiStr> for HeaderName {
    fn eq(&self, other: &AsciiStr) -> bool {
        self.name == other
    }
}

impl HeaderName {

    /// validates if the header name is valid
    ///
    /// by only allowing names in "snake case" no case
    /// insensitive comparsion or case conversion is needed
    /// for header names
    fn validate_name( name: &AsciiStr ) -> Result<()> {
        let mut begin_of_word = true;
        if name.len() < 1 {
            bail!( "header names must consist of at last 1 character" );
        }

        for ch in name.as_str().chars() {
            if !is_ftext( ch ) {
                bail!( "invalide header name" )
            }
            match ch {
                'a'...'z' => {
                    if begin_of_word {
                        bail!( "the case of header names is restricted to the shema used in RFC 5322: {}", name );
                    }
                },
                'A'...'Z' => {
                    if begin_of_word {
                        begin_of_word = false;
                    } else {
                        bail!( "the case of header names is restricted to the shema used in RFC 5322: {}", name );
                    }
                },
                '0'...'9' => {
                    begin_of_word = false;
                },
                ch => {
                    if ch < '!' || ch > '~' || ch == ':' {
                        bail!( "invalide character for header field name in {:?}", name )
                    }
                    begin_of_word = true;
                }

            }

        }
        Ok( () )
    }
}


#[cfg(test)]
mod test {
    use super::*;


    #[test]
    fn valide_header_names() {
        let valid_cases = &[
            "Date",
            "Some-Header",
            "33",
            "Some34",
            // even trough they seem wrong the email standard only states
            // header field names have to be at last one char and can
            // only consist of printable US-ACII chars without :
            // meaning e.g. "<3" is as valide as "3*4=12"
            "-33-",
            "---",
            "<3+Who-Cares&44",
            "(3*4=12)^[{~}]"
        ];
        for case in valid_cases.iter() {
            assert_ok!( HeaderName::validate_name( AsciiStr::from_ascii( case ).unwrap() ) );
        }
    }

    #[test]
    fn invalide_header_names() {
        // we only alow "snake case" like names to not have to do
        // case insensitive comparsion in hashmap lookups
        let invalid_cases = &[
            "ID",
            "DaD",
            "ans",
            "all-lower-calse",
            "ALL-UPPER-CASE",
            "",
            "a:b",
            ":",
            "-:-",
            "Message Id",
            " Leading-Ws",
            "Message\tId",
            "Null\0Msg"
        ];
        for case in invalid_cases.iter() {
            assert_err!( HeaderName::validate_name( AsciiStr::from_ascii( case ).unwrap() ), case );
        }
    }
}


