use error::*;
use codec::{ MailEncodable, MailEncoder };

use grammar::is_atext;
use grammar::encoded_word::EncodedWordContext;

use data::{
    FromInput,
    Input,
    EncodedWord,
    Encoding as ECWEncoding,
    QuotedString
};


use super::CFWS;



#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Word {
    pub left_padding: Option<CFWS>,
    pub input: Input,
    pub right_padding: Option<CFWS>
}

impl FromInput for Word {

    fn from_input( input: Input ) -> Result<Self> {
        //FEATURE_TODO(fail_fast): check if input contains a CTL char,
        //  which is/>>should<< always be an error (through in the standard you could but should
        //  not have them in encoded words)
        Ok( Word { left_padding: None, input, right_padding: None } )
    }
}

impl Word {

    pub fn pad_left( &mut self, padding: CFWS) {
        self.left_padding = Some( padding )
    }

    pub fn pad_right( &mut self, padding: CFWS) {
        self.right_padding = Some( padding )
    }

}


/// As word has to be differently encoded, depending on the context it
/// appears in it cannot implement MailEncodable, instead we have
/// a function which can be used by type containing it which (should)
/// implement MailEncodable
///
/// If `ecw_ctx` is `None` the word can not be encoded as a "encoded-word",
/// if it is `Some(context)` the `context` represents in which context the
/// word does appear, which changes some properties of the encoded word.
///
/// NOTE: != encoded-word, through it might create an encoded-word
pub fn do_encode_word<E: MailEncoder>(
    word: &Word,
    encoder: &mut E,
    ecw_ctx: Option<EncodedWordContext>,
) -> Result<()> {

    if let Some( pad ) = word.left_padding.as_ref() {
        pad.encode( encoder )?;
    }

    let input: &str = &*word.input;

    let ok = (!input.starts_with("=?")) && word.input.chars()
        .all( |ch| is_atext( ch, encoder.mail_type() ) );

    if ok {
        encoder.write_str_unchecked( input )
    } else {
        if let Some( ecw_ctx ) = ecw_ctx {
            EncodedWord::write_into( encoder, input, ECWEncoding::QuotedPrintable, ecw_ctx );
        } else {
            QuotedString::write_into( encoder, input )?;
        }
    }

    if let Some( pad ) = word.right_padding.as_ref() {
        pad.encode( encoder )?;
    }
    Ok( () )
}


#[cfg(test)]
mod test {
    use super::*;
    use super::super::{ FWS as Fws };
    use grammar::MailType;
    use codec::test_utils::*;

    #[test]
    fn encode_pseudo_encoded_words() {
        let word = Word::from_input( "=?".into() ).unwrap();
        let mut encoder = TestMailEncoder::new( MailType::Ascii );
        do_encode_word( &word, &mut encoder, Some( EncodedWordContext::Text ) ).unwrap();
        assert_eq!(
            vec![ LinePart( "=?utf8?Q?=3D=3F?=" ) ],
            encoder.into_state_seq()
        )
    }

    #[test]
    fn encode_word() {
        let word = Word::from_input( "a↑b".into() ).unwrap();
        let mut encoder = TestMailEncoder::new( MailType::Ascii );
        do_encode_word( &word, &mut encoder, Some( EncodedWordContext::Text ) ).unwrap();
        assert_eq!(
            vec![ LinePart( "=?utf8?Q?a=E2=86=91b?=" ) ],
            encoder.into_state_seq()
        )
    }

    #[test]
    fn encode_fails() {
        let word = Word::from_input( "a↑b".into() ).unwrap();
        let mut encoder = TestMailEncoder::new( MailType::Ascii );
        let res = do_encode_word( &word, &mut encoder, None );
        assert_eq!( false, res.is_ok() );
    }

    #[test]
    fn quoted_fallback() {
        let word = Word::from_input( "a\"b".into() ).unwrap();
        let mut encoder = TestMailEncoder::new( MailType::Ascii );
        do_encode_word( &word, &mut encoder, None ).unwrap();
        assert_eq!(
            vec![ LinePart(r#""a\"b""#) ],
            encoder.into_state_seq()
        )
    }

    #[test]
    fn encode_word_padding() {
        let words = &[
            ( Word {
                left_padding: None,
                input: "abc".into(),
                right_padding: None,
            }, vec![
                LinePart( "abc" )
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( Fws ) ),
                input: "abc".into(),
                right_padding: None,
            }, vec![
                FWS,
                LinePart( "abc" )
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( Fws ) ),
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( Fws ) ),
            }, vec![
                FWS,
                LinePart( "abc" ),
                FWS
            ] ),
            ( Word {
                left_padding: None,
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( Fws ) ),
            }, vec![
                LinePart( "abc" ),
                FWS
            ] )
        ];

        for &( ref word, ref expection) in words.iter() {
            let mut encoder = TestMailEncoder::new( MailType::Ascii );
            do_encode_word( word, &mut encoder, None ).unwrap();
            let seq = encoder.into_state_seq();
            assert_eq!(
                expection,
                &seq
            );
        }
    }


}