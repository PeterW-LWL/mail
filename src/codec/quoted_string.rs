use std::borrow::Cow;

use error::*;
use grammar::{
    MailType,
    is_ascii,
    is_vchar,
    is_ws,
    is_qtext
};

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
pub fn quote_if_needed<'a, FN>(input: &'a str, valid_without_quoting: FN, allowed_mail_type: MailType )
    -> Result<(MailType, Cow<'a, str>)>
    where FN: FnMut(char) -> bool
{
    let (mut ascii, offset) = scan_ahead(input, valid_without_quoting, allowed_mail_type)?;
    //NOTE: no need to check ascii scan_ahead errors if !ascii && allowed_mail_type == Ascii
    if offset == input.len() {
        return Ok((mailtype_from_is_ascii_bool(ascii), Cow::Borrowed(input)))
    }

    let (ok, rest) = input.split_at(offset);
    //just guess half of the remaining chars needs escaping
    let mut out = String::with_capacity((rest.len() as f64 * 1.5) as usize);
    out.push('\"');
    out.push_str(ok);

    let ascii_only = allowed_mail_type == MailType::Ascii;
    for ch in rest.chars() {
        if ascii && !is_ascii( ch ) {
            if ascii_only {
                bail!("excepted ascii only characters got: {:?}", ch);
            } else {
                ascii = false;
            }
        }
        if is_qtext( ch, MailType::Internationalized ) {
            out.push( ch )
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

    Ok( (mailtype_from_is_ascii_bool(ascii), Cow::Owned(out)) )
}


#[inline]
fn mailtype_from_is_ascii_bool(is_ascii: bool) -> MailType {
    if is_ascii {
        MailType::Ascii
    } else {
        MailType::Internationalized
    }
}

fn scan_ahead<FN>(inp: &str, mut valid_without_quoting: FN, tp: MailType) -> Result<(bool, usize)>
    where FN: FnMut(char) -> bool
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
        if !valid_without_quoting(ch) {
            return Ok((ascii, offset))
        }
    }
    Ok((ascii, inp.len()))
}

#[cfg(test)]
mod test {
    use grammar::{ is_vchar, is_token_char, is_qtext};
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
        let (mt, res) = assert_ok!(
            quote_if_needed("simple", is_token_char, MailType::Ascii));
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
            let (mt, res) = assert_ok!(quote_if_needed(unquoted, is_token_char, MailType::Ascii));
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
        let res = quote_if_needed("l↓r", is_token_char, mt);
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
}