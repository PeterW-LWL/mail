

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum MailType {
    Ascii,
    Mime8BitEnabled,
    Internationalized
}

impl MailType {
    pub fn is_internationalized(&self) -> bool {
        *self == MailType::Internationalized
    }
    pub fn supports_8bit_bodies( &self ) -> bool {
        use self::MailType::*;
        match *self {
            Ascii => false,
            Mime8BitEnabled => true,
            Internationalized => true
        }
    }
}

/// ftext as defined by RFC 5322
///
/// which is: printable US-ASCII characters not includign `:`
///  => 0x21-0x39 / 0x3B-0x7E
///  => '!'...'9' / ';'...'~'
///  => <0x7F && != 0x3A
#[inline(always)]
pub fn is_ftext( ch: char ) -> bool {
    (ch as u32) < 127 && ch != ':'
}

///WS as defined by RFC 5234
#[inline(always)]
pub fn is_ws( ch: char ) -> bool {
    // is not limited to ascii ws
    //ch.is_whitespace()
    //WSP            =  SP / HTAB
    ch == ' ' || ch == '\t'
}

#[inline(always)]
pub fn is_space( ch: char ) -> bool {
    ch == ' '
}

#[inline(always)]
pub fn is_ascii( ch: char ) -> bool {
    (ch as u32) < 128
}

#[inline(always)]
pub fn is_ascii_vchar( ch: char ) -> bool {
    let u32_ch = ch as u32;
    32 < u32_ch && u32_ch <= 126
}

//VCHAR as defined by RFC 5243
#[inline(always)]
pub fn is_vchar( ch: char, mt: MailType ) -> bool {
    is_ascii_vchar( ch ) || ( mt == MailType::Internationalized && !is_ascii( ch ) )
}


//can be quoted in a quoted string (internalized) based on RFC ... and RFC ...
#[inline(always)]
pub fn is_quotable( ch: char, tp: MailType ) -> bool {
    is_vchar( ch, tp) || is_ws( ch )
}

///any whitespace (char::is_whitespace
#[inline(always)]
pub fn is_any_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}

//ctext as defined by RFC 5322
pub fn is_ctext( ch: char, mt: MailType  ) -> bool {
    match ch {
        '!'...'\'' |
        '*'...'[' |
        ']'...'~' => true,
        // obs-ctext
        _ => mt == MailType::Internationalized && !is_ascii( ch )
    }
}

/// check if a char is a especial (based on RFC 5322)
pub fn is_special(ch: char ) -> bool {
    match ch {
        '(' | ')' |
        '<' | '>' |
        '[' | ']' |
        ':' | ';' |
        '@' | '\\'|
        ',' | '.' |
        '"' => true,
        _ => false
    }
}


/// check if a char is an tspecial (based on RFC 2045)
pub fn is_tspecial( ch: char ) -> bool {
    match ch {
        '(' | ')' |
        '<' | '>' |
        '@' | ',' |
        ';' | ':' |
        '\\'| '"' |
        '/' | '[' |
        ']' | '?' |
        '=' => true,
        _ => false
    }
}



/// atext as defined by RFC 5322
#[inline(always)]
pub fn is_atext( ch: char, tp: MailType  ) -> bool {
    is_vchar( ch, tp) && !is_special( ch )
}

///dtext as defined by RFC 5322
#[inline(always)]
pub fn is_dtext( ch: char , mt: MailType ) -> bool {
    match ch as u32 {
        33...90 |
        94...126 => true,
        _ => mt == MailType::Internationalized && !is_ascii( ch )
    }
}

//qtext as defined by RFC 5322
pub fn is_qtext( ch: char, mt: MailType ) -> bool {
    match ch {
        //not ' ' [d:32]
        '!' |
        //not '"' [d:34]
        '#'...'[' |
        //not '\\' [d:92]
        ']'...'~' => true,
        //obs-qtext
        _ => mt == MailType::Internationalized && !is_ascii( ch )
    }
}

/// is it a CTL (based on RFC 822)
///
/// # Note
/// the standard specifies `'\t'` as a CTL but not `' '`
/// but both `'\t'` and `' '` are LWSP-char i.e. semantically
/// space i.e. _semantically equivalent_.
#[inline(always)]
pub fn is_ctl( ch: char ) -> bool {
    (ch as u32) < 32
}

/// check if a char is an token char (based on RFC 2045)
#[inline(always)]
pub fn is_token_char( ch: char ) -> bool {
    is_ascii( ch ) && !is_ctl( ch ) && !is_tspecial( ch ) && ch != ' '
}


#[inline(always)]
pub fn is_especial( ch: char ) -> bool {
    match ch {
        '(' | ')' |
        '<' | '>' |
        '@' | ',' |
        ';' | ':' |
        '"' | '/'|
        '[' | ']' |
        '?' | '.' |
        '=' => true,
        _ => false
    }
}

pub fn is_token(s: &str) -> bool {
    0 < s.len() && s.chars().all(is_token_char)
}

//
//pub fn is_dot_atom_text( text: &str, mt: MailType ) -> bool {
//    use nom::IResult;
//    use self::parse::recognize_dot_atom_text;
//
//    let res = tuple!( text,
//        call!( recognize_dot_atom_text, mt ),
//        eof!()
//    );
//
//    match res {
//        IResult::Done(_, _) => true,
//        _ => false
//    }
//}

//pub mod parse {
//    use nom::IResult;
//    use super::{ is_atext, MailType };
//
//    pub fn recognize_dot_atom_text( input: &str, mt: MailType ) -> IResult<&str, &str> {
//        recognize!( input, tuple!(
//            take_while1!( call!( is_atext, mt ) ),
//            many0!( tuple!(
//                char!( "." ),
//                take_while1!( call!( is_atext, mt ) )
//            ) )
//        ) )
//    }
//
//}
//TODO thisshould be some where else I think
// (but it is used by `1. codec`, `2. components` )
/// based on RFC 2047
pub mod encoded_word {
    use nom;
    use error::*;
    use super::{  is_especial, is_ascii_vchar };

    pub const MAX_ECW_LEN: usize = 75;
    // the overhead from: `=?<>?<>?<>?=` not including the length of the `<>`
    pub const ECW_SEP_OVERHEAD: usize = 6;

    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    pub enum EncodedWordContext {
        Phrase,
        Text,
        Comment
    }

    impl EncodedWordContext {

        fn char_validator( &self ) -> fn(char) -> bool {
            use self::EncodedWordContext::*;
            match *self {
                Phrase => valid_char_in_ec_in_phrase,
                Text => is_encoded_word_char,
                Comment => valid_char_in_ec_in_comment,
            }
        }
    }


    pub fn is_encoded_word( word: &str, ctx: EncodedWordContext ) -> bool {
        try_parse_encoded_word_parts( word, ctx ).is_ok()
    }

    pub fn try_parse_encoded_word_parts( word: &str, ctx: EncodedWordContext )
                                         -> Result<(&str, &str, &str)>
    {
        let char_validator = ctx.char_validator();
        // Note we could get a possible speed up by making rustc generate
        // a different function for each Context, inlining ALL char tests
        let res = do_parse!(
            word,
            char!( '=' ) >>
            char!( '?' ) >>
            charset: take_while!( is_ew_token_char ) >>
            char!( '?' ) >>
            encoding: take_while!( is_ew_token_char ) >>
            char!( '?' ) >>
            text: take_while!( char_validator ) >>
            char!( '?' ) >>
            char!( '=' ) >>
            eof!() >>
            (charset, encoding, text)
        );

        match res {
            nom::IResult::Done( rest, result ) => {
                assert!( rest.len() == 0, "[BUG] used nom::eof!() but rest.len() > 0" );
                Ok( result )
            },
            nom::IResult::Incomplete( .. ) => bail!( "incomplete encoded word: {:?}", word ),
            nom::IResult::Error( e ) => bail!( "malformed encoded word: {:?}, {:?}", word, e )
        }
    }

    fn is_encoded_word_char( ch: char ) -> bool {
        is_ascii_vchar( ch ) && ch != '?'
    }


    fn valid_char_in_ec_in_comment( ch: char ) -> bool {
        is_encoded_word_char( ch ) && !( ch == '(' || ch == ')' || ch == '"' )
    }

    fn is_ew_token_char( ch: char ) -> bool {
        is_ascii_vchar( ch ) && !is_especial( ch )
    }




    fn valid_char_in_ec_in_phrase( ch: char ) -> bool {
        match ch {
            '0'...'9' |
            'a'...'z' |
            'A'...'Z' |
            '!' | '*' |
            '+' | '-' |
            '/' | '=' |
            '_' => true,
            _ => false
        }
    }

}

pub fn is_quoted_string( qstr: &str, tp: MailType ) -> bool {
    let mut iter = qstr.chars();
    if let Some('"') = iter.next() {} else { return false }
    let mut next = iter.next();
    while let Some(ch) = next {
        match ch {
            '\\' => {
                if let Some(next_char) = iter.next() {
                    if !( is_vchar( next_char, tp ) || is_ws( next_char ) ) {
                        return false;
                    }
                } else {
                    return false;
                }
            },
            '"' => {
                if iter.next().is_none() {
                    return true;
                } else {
                    return false;
                }
            }
            ch => {
                if !is_qtext( ch, tp ) {
                    return false
                }
            }
        }
        next = iter.next()
    }

    // The only true return if we have a '"' followed by iter.next().is_none()
    return false;
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn _is_ascii_vchar() {
        assert_eq!(false, is_ascii_vchar('\x7f'));
        for bad_char in b'\0'..b' ' {
            if is_ascii_vchar(bad_char as char) {
                panic!("{:?} should not be a VCHAR", bad_char);
            }
        }
        for good_char in b'!'..(b'~'+1) {
            if !is_ascii_vchar(good_char as char) {
                panic!("{:?} should be a VCHAR", good_char as char);
            }
        }
    }

    #[test]
    fn htap_is_ctl_space_is_not() {
        assert_eq!(true, is_ctl('\t'));
        assert_eq!(false, is_ctl(' '));
    }

    #[test]
    fn is_toke_empty() {
        assert_eq!(false, is_token(""));
    }
}

