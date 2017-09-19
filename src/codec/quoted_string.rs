use error::*;

use grammar::{
    MailType,
    is_ascii,
    is_vchar,
    is_ws,
    is_qtext
};


/// quotes the input string (RFC 5322/6532/822)
///
/// additionally to quoting a string the mailtype required to
/// use the quoted string is returned. Return values with
/// mailtype `Ascii` are compatible with RFC 5322/822 mails.
/// If the mailtype is `Internationalized` then a internationalized
/// mail is required (RFC 6532 extends the `qtext` grammar)
///
/// The quoting process can fail if characters are contained,
/// which can not appear in a quoted string independent of
/// mail type. Thish are chars which are neither `qtext`,`vchar`
/// nor WS (`' '` and `'\t'`). Which are basically only 0x7F (DEL)
/// and the characters < 0x20 (`' '`) except 0x9 (`'\t'`).
pub fn quote<'a>( input: &str ) -> Result<(MailType, String)> {
    let mut ascii = true;
    let mut out = String::new();
    out.push( '"' );
    for char in input.chars() {
        if ascii { ascii = is_ascii( char ) }
        if is_qtext( char, MailType::Internationalized ) {
            out.push( char )
        } else {
            // we do not have to escape ' ' and '\t' (but could)
            if is_ws( char ) {
                out.push( char )
            } else if is_vchar( char, MailType::Internationalized ) {
                out.push( '\\' );
                out.push( char );
            } else {
                // char: 0-31,127 expect 9 ('\t')
                bail!( "can not quote char: {:?}", char );
            }
        }
    }
    out.push( '"' );
    let mail_type =
        if ascii { MailType::Ascii }
        else { MailType::Internationalized };

    Ok((mail_type, out))
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn quote_ascii() {
        let data = &[
            ("this is simple", "\"this is simple\""),
            ("also\tsimple", "\"also\tsimple\""),
            ("with quotes\"<-", "\"with quotes\\\"<-\""),
            ("with slash\\<-", "\"with slash\\\\<-\"")
        ];
        for &(unquoted, quoted) in data.iter() {
            let (mail_type, got_quoted) = assert_ok!(quote(unquoted));
            assert_eq!(MailType::Ascii, mail_type);
            assert_eq!(quoted, got_quoted);
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
            let (mail_type, got_quoted) = assert_ok!(quote(unquoted));
            assert_eq!(MailType::Internationalized, mail_type);
            assert_eq!(quoted, got_quoted);
        }
    }

    #[test]
    fn no_del() {
        assert_err!(quote("\x7F"));
    }

    #[test]
    fn no_ctl() {
        let mut text = String::with_capacity(1);
        let bad_chars = (b'\0'..b' ').filter(|&b| b != b'\t' ).map(|byte| byte as char);
        for char in bad_chars {
            text.clear();
            text.insert(0, char);
            assert_err!(quote(&*text));
        }
    }
}