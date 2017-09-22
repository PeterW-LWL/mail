use std::borrow::Cow;

use error::*;
use grammar::{
    MailType,
    is_ascii,
    is_vchar,
    is_ws,
    is_qtext,
    //used by glue
    is_atext,
    is_token_char
};


/// Used to determine
/// 1. if the string needs quoting
/// 2. where the first char which migth require quoting appear, to efficiently copy it over
///
/// Note that a string can compare only of chars which do not need quoting in a
/// quoted string but still requires quoting, e.g. a `"a."` local-part is not
/// valide but an `"\"a.\""` local-part is.
pub trait ValidWithoutQuotationCheck {
    /// should return true if the next char is valid without quotation
    fn next_char(&mut self, ch: char) -> bool;

    /// Called after the last char was passed to `next_char`.
    /// It should return true if the whole string is valid without
    /// quotation _assuming_ that before all chars where passed in
    /// order to `next_char` and all calls to `next_char` returned
    /// true.
    ///
    /// This can be used to checks not possible with on a char by
    /// char basis e.g. if it does not end in a `.`.
    ///
    /// Note that because it is only called after one iteration,
    /// validation should be done, if possible, in the `next_char`
    /// method.
    fn end(&mut self, _all: &str) -> bool { true }
}

impl<T> ValidWithoutQuotationCheck for T
    where T: FnMut(char) -> bool
{
    fn next_char(&mut self, ch: char) -> bool {
        (self)(ch)
    }
}


/// quotes the input string
///
/// basically calls `quote_if_needed(input, |_|false, MailType::Internationalized)`
#[inline]
pub fn quote(input: &str) -> Result<(MailType, String)> {
    let (mt, res) = quote_if_needed(input, |_|false, MailType::Internationalized)?;
    Ok( match res {
        Cow::Owned(owned) =>  (mt, owned),
        _ => unreachable!("[BUG] the string should have been quoted but wasn't")
    } )
}

/// quotes the input string if needed(RFC 5322/6532/822)
///
/// The `valid_without_quoting` parameter accepts a function,
/// which chould _only_ returns if the char is really valid
/// without quoting. So this function should never return true
/// for e.g. `\0`. Use this function if some characters are
/// only valid in a quoted-string context.
///
/// If the `allowed_mail_type` parameter is set to `Ascii`
/// the algorithm will return a error if it stumbles over
/// a non-ascii character, elese it will just indicate the
/// appearence of one through the returned mail type. Note
/// that if you set the `allowed_mail_type` to `Internationalized`
/// the function still can returns a `Ascii` mail type as it
/// is compatible with `Internationalized`
///
///
/// additionally to quoting a string the mail type required to
/// use the quoted string is returned. Return values with
/// mail type `Ascii` are compatible with RFC 5322/822 mails.
/// If the mailtype is `Internationalized` then a internationalized
/// mail is required (RFC 6532 extends the `qtext` grammar)
///
/// The quoting process can fail if characters are contained,
/// which can not appear in a quoted string independent of
/// mail type. Thish are chars which are neither `qtext`,`vchar`
/// nor WS (`' '` and `'\t'`). Which are basically only 0x7F (DEL)
/// and the characters < 0x20 (`' '`) except 0x9 (`'\t'`).
///
/// Note that if the `valid_without_quoting` function states a CTL
/// char is valid without quoting and there is no char requiring a
/// quoted string, no quoting will be done and therefore no error
/// will be returned even through it contains a CTL.
///
pub fn quote_if_needed<'a, FN>(
    input: &'a str,
    mut valid_without_quotation: FN,
    allowed_mail_type: MailType
) -> Result<(MailType, Cow<'a, str>)>
    where FN: ValidWithoutQuotationCheck
{
    let valid_without_quoting = &mut valid_without_quotation;

    let (ascii, offset) = scan_ahead(input, valid_without_quoting, allowed_mail_type)?;
    if offset == input.len() && valid_without_quoting.end(input) {
        //NOTE: no need to check ascii scan_ahead errors if !ascii && allowed_mail_type == Ascii
        return Ok((mailtype_from_is_ascii_bool(ascii), Cow::Borrowed(input)))
    }

    let (ascii, out) = _quote(input, ascii, allowed_mail_type, offset)?;

    Ok( (mailtype_from_is_ascii_bool(ascii), Cow::Owned(out)) )
}

fn _quote(
        input: &str,
        was_ascii: bool,
        allowed_mail_type: MailType,
        start_escape_check_from: usize
) -> Result<(bool, String)>
{
    let ascii_only = allowed_mail_type == MailType::Ascii;
    debug_assert!(!(ascii_only && !was_ascii));

    let (ok, rest) = input.split_at(start_escape_check_from);
    //just guess half of the remaining chars needs escaping
    let mut out = String::with_capacity((rest.len() as f64 * 1.5) as usize);
    out.push('\"');
    out.push_str(ok);

    let mut ascii = was_ascii;
    for ch in rest.chars() {
        if ascii && !is_ascii( ch ) {
            if ascii_only {
                bail!("excepted ascii only characters got: {:?}", ch);
            } else {
                ascii = false;
            }
        }

        // we are no asci specific in this part of the algorithm (and it's fine)
        if is_qtext( ch, MailType::Internationalized ) {
            out.push( ch );
        } else {
            // we do not have to escape ' ' and '\t' (but could)
            if is_ws( ch ) {
                out.push( ch )
            } else if is_vchar( ch, MailType::Internationalized ) {
                out.push( '\\' );
                out.push( ch );
            } else {
                // char: 0-31,127 expect 9 ('\t')
                bail!( "can not quote char: {:?}", ch );
            }
        }
    }
    out.push( '"' );
    Ok((ascii, out))

}


#[inline]
fn mailtype_from_is_ascii_bool(is_ascii: bool) -> MailType {
    if is_ascii {
        MailType::Ascii
    } else {
        MailType::Internationalized
    }
}

fn scan_ahead<FN>(inp: &str, valid_without_quoting: &mut FN, tp: MailType) -> Result<(bool, usize)>
    where FN: ValidWithoutQuotationCheck
{
    let ascii_only = tp == MailType::Ascii;
    let mut ascii = true;
    for (offset, ch) in inp.char_indices() {
        if ascii && !is_ascii(ch) {
            if ascii_only {
                bail!("excepted ascii only characters got: {:?}", ch);
            } else {
                ascii = false;
            }
        }
        if !valid_without_quoting.next_char(ch) {
            return Ok((ascii, offset))
        }
    }
    Ok((ascii, inp.len()))
}


//NOTE: this struct can not be placed in grammar as it specific
// to quoted_string, but it can be placed here in the
// mail-codec <==> quted_string glue, as the glue can depend on
// grammar
pub struct DotAtomTextCheck {
    can_be_dot: bool,
    mail_type: MailType
}

impl DotAtomTextCheck {
    pub fn new(mail_type: MailType) -> DotAtomTextCheck {
        DotAtomTextCheck {
            mail_type,
            can_be_dot: false,
        }
    }
}

impl ValidWithoutQuotationCheck for DotAtomTextCheck {
    fn next_char(&mut self, ch: char) -> bool {
        if ch == '.' {
            if self.can_be_dot {
                self.can_be_dot = false;
                true
            } else {
                false
            }
        } else {
            self.can_be_dot = true;
            is_atext(ch, self.mail_type)
        }
    }

    fn end(&mut self, _all: &str) -> bool {
        let can_be_dot = self.can_be_dot;
        //reset it, just to be on the safe side
        self.can_be_dot = false;
        // it's only false if "empty" or "dot at end" which are both invalid
        can_be_dot == true
    }
}


pub struct TokenCheck;

impl ValidWithoutQuotationCheck for TokenCheck {
    fn next_char(&mut self, ch: char) -> bool {
        is_token_char(ch)
    }
    fn end(&mut self, all: &str) -> bool {
        0 < all.len()
    }
}

#[cfg(test)]
mod test {
    use grammar::{ is_vchar, is_qtext};
    use super::*;


    #[test]
    fn quote_ascii() {
        let mti = MailType::Internationalized;
        let data = &[
            ("this is simple", "\"this is simple\""),
            ("also\tsimple", "\"also\tsimple\""),
            ("with quotes\"<-", "\"with quotes\\\"<-\""),
            ("with slash\\<-", "\"with slash\\\\<-\"")
        ];
        for &(unquoted, quoted) in data.iter() {
            let (mail_type, got_quoted) = assert_ok!(
                quote_if_needed( unquoted, |ch| is_vchar(ch, mti), mti));
            assert_eq!(MailType::Ascii, mail_type);
            assert_eq!(quoted, &*got_quoted);
        }
    }

    #[test]
    fn quote_utf8() {
        let data = &[
            ("has → uft8", "\"has → uft8\""),
            ("also\t→\tsimple", "\"also\t→\tsimple\""),
            ("with→quotes\"<-", "\"with→quotes\\\"<-\""),
            ("with→slash\\<-", "\"with→slash\\\\<-\"")
        ];
        for &(unquoted, quoted) in data.iter() {
            let res = quote_if_needed( unquoted, |_|false, MailType::Internationalized );
            let (mail_type, got_quoted) = assert_ok!(res);
            assert_eq!(MailType::Internationalized, mail_type);
            assert_eq!(quoted, &*got_quoted);
        }
    }
    
    #[test]
    fn no_quotation_needed_ascii() {
        let res = quote_if_needed("simple", TokenCheck, MailType::Ascii);
        let (mt, res) = assert_ok!(res);
        assert_eq!(MailType::Ascii, mt);
        assert_eq!("simple", &*res);
        let is_borrowed = if let Cow::Borrowed(_) = res { true } else { false };
        assert_eq!(true, is_borrowed);
    }

    #[test]
    fn no_quotation_needed_utf8() {
        let mt = MailType::Internationalized;
        let (mt, res) = assert_ok!(
            quote_if_needed("simp↓e", |ch| is_qtext(ch, mt), mt));
        assert_eq!(MailType::Internationalized, mt);
        assert_eq!("simp↓e", &*res);
        let is_borrowed = if let Cow::Borrowed(_) = res { true } else { false };
        assert_eq!(true, is_borrowed);
    }

    #[test]
    fn no_del() {
        assert_err!(quote_if_needed("\x7F", |_|false, MailType::Ascii));
    }

    #[test]
    fn no_ctl() {
        let mut text = String::with_capacity(1);
        let bad_chars = (b'\0'..b' ').filter(|&b| b != b'\t' ).map(|byte| byte as char);
        for char in bad_chars {
            text.clear();
            text.insert(0, char);
            assert_err!(quote_if_needed(&*text, |_|false, MailType::Ascii));
        }
    }

    #[test]
    fn quote_always_quotes() {
        assert_eq!(
            (MailType::Ascii, "\"simple\"".to_owned()),
            assert_ok!(quote("simple"))
        );
    }

    #[test]
    fn using_valid_without_quoting() {
        let data = &[
            ("not@a-token", "\"not@a-token\"", true),
            ("not a-token", "\"not a-token\"", true),
            ("a-token-it-is", "a-token-it-is", false)
        ];
        for &(unquoted, exp_res, quoted) in data.iter() {
            let res = quote_if_needed(unquoted, TokenCheck, MailType::Ascii);
            let (mt, res) = assert_ok!(res);
            assert_eq!(MailType::Ascii, mt);
            if quoted {
                let owned: Cow<str> = Cow::Owned(exp_res.to_owned());
                assert_eq!(owned, res);
            } else {
                assert_eq!(Cow::Borrowed(exp_res), res);
            }
        }
    }

    #[test]
    fn quotes_utf8() {
        let mt = MailType::Internationalized;
        let res = quote_if_needed("l↓r", TokenCheck, mt);
        let res = assert_ok!(res);
        let was_quoted = if let &Cow::Owned(..) = &res.1 { true } else { false };
        assert_eq!( true, was_quoted );
    }

    #[test]
    fn error_with_quotable_utf8_but_ascii_only() {
        let res = quote_if_needed("l→r",
                                  |ch|is_qtext(ch, MailType::Internationalized),
                                  MailType::Ascii);
        assert_err!(res);
    }

    #[test]
    fn error_with_quotable_utf8_but_ascii_only_2() {
        let res = quote_if_needed("l→r",
                                  |ch|is_qtext(ch, MailType::Ascii),
                                  MailType::Ascii);
        assert_err!(res);
    }

    #[test]
    fn check_end_is_used() {
        let mt = MailType::Ascii;
        let res = quote_if_needed("a.", DotAtomTextCheck::new(mt), mt);
        let (got_mt, quoted) = assert_ok!(res);
        assert_eq!(MailType::Ascii, got_mt);
        assert_eq!("\"a.\"", quoted);
    }

    #[test]
    fn is_dot_atom_text_check() {
        let checks = [
            ("simepl", true),
            ("more.complex", true),
            (".um", false),
            ("a..b", false),
            //we only test with next_char so it's ok
            ("a.", true)
        ];

        for &(text, ok) in checks.iter() {
            let mut check = DotAtomTextCheck::new(MailType::Ascii);
            let res = text.chars().all(|ch| check.next_char(ch) );
            if res != ok {
                panic!("expected `chars().all(closure)` on {:?} to return {:?}", text, ok);
            }
        }
    }

    #[test]
    fn dot_atom_text_no_tailing_dot() {
        let text = "a.";
        let mut check = DotAtomTextCheck::new(MailType::Ascii);
        let res = text.chars().all(|ch| check.next_char(ch) );
        assert_eq!(true, res);
        assert_eq!(false, check.end(text))
    }

    #[test]
    fn dot_atom_text_utf8() {
        let text = "a↓.→";
        let mut check = DotAtomTextCheck::new(MailType::Internationalized);
        let res = text.chars().all(|ch| check.next_char(ch) );
        assert_eq!(true, res);
        assert_eq!(true, check.end(text))
    }
}