use error::*;
use codec::{ 
    EncodedWordEncoding,
    EncodableInHeader, EncodeHeaderHandle
};

use grammar::is_atext;
use grammar::encoded_word::EncodedWordContext;

use data::{
    FromInput,
    Input,
    EncodedWord,
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

    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        //FEATURE_TODO(fail_fast): check if input contains a CTL char,
        //  which is/>>should<< always be an error (through in the standard you could but should
        //  not have them in encoded words)
        Ok( Word { left_padding: None, input: input.into(), right_padding: None } )
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
/// appears in it cannot implement EncodableInHeader, instead we have
/// a function which can be used by type containing it which (should)
/// implement EncodableInHeader
///
/// If `ecw_ctx` is `None` the word can not be encoded as a "encoded-word",
/// if it is `Some(context)` the `context` represents in which context the
/// word does appear, which changes some properties of the encoded word.
///
/// NOTE: != encoded-word, through it might create an encoded-word
pub fn do_encode_word<'a,'b: 'a>(
    word: &'a Word,
    handle: &'a mut EncodeHeaderHandle<'b>,
    ecw_ctx: Option<EncodedWordContext>,
) -> Result<()> {

    if let Some( pad ) = word.left_padding.as_ref() {
        pad.encode( handle )?;
    }

    let input: &str = &*word.input;

    let ok = (!input.starts_with("=?")) && word.input.chars()
        .all( |ch| is_atext( ch, handle.mail_type() ) );

    if ok {
        handle.write_str_unchecked( input )?;
    } else {
        if let Some( ecw_ctx ) = ecw_ctx {
            EncodedWord::write_into( handle, input,
                                     EncodedWordEncoding::QuotedPrintable, ecw_ctx );
        } else {
            QuotedString::write_into( handle, input )?;
        }
    }

    if let Some( pad ) = word.right_padding.as_ref() {
        pad.encode( handle )?;
    }
    Ok( () )
}


#[cfg(test)]
mod test {
    use std::mem;

    use grammar::MailType;
    use codec::{ Encoder, VecBodyBuf, EncodableClosure};
    use codec::Token::*;

    use super::*;
    use super::super::FWS;

    ec_test!{encode_pseudo_encoded_words, {
        let word = Word::from_input( "=?" )?;
        EncodableClosure(move |handle: &mut EncodeHeaderHandle| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        NowStr,
        Text "=?utf8?Q?=3D=3F?="
    ]}

    ec_test!{encode_word, {
        let word = Word::from_input( "a↑b" )?;
        EncodableClosure(move |handle: &mut EncodeHeaderHandle| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        NowStr,
        Text "=?utf8?Q?a=E2=86=91b?="
    ]}


    #[test]
    fn encode_fails() {
        let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
        let mut handle = encoder.encode_header_handle();
        let word = Word::from_input( "a↑b" ).unwrap();
        assert_err!(do_encode_word( &word, &mut handle, None ));
        handle.undo_header();
    }


    ec_test!{quoted_fallback, {
        let word = Word::from_input( "a\"b" )?;
        EncodableClosure(move |handle: &mut EncodeHeaderHandle| {
            do_encode_word( &word, handle, None )
        })
    } => ascii => [
        NowStr,
        Text r#""a\"b""#
    ]}


    #[test]
    fn encode_word_padding() {
        let words = &[
            ( Word {
                left_padding: None,
                input: "abc".into(),
                right_padding: None,
            }, vec![
                NowStr,
                Text("abc".into())
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( FWS) ),
                input: "abc".into(),
                right_padding: None,
            }, vec![
                MarkFWS, NowChar, Text(" ".into()),
                NowStr,
                Text("abc".into())
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( FWS ) ),
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( FWS ) ),
            }, vec![
                MarkFWS, NowChar, Text(" ".into()),
                NowStr,
                Text("abc".into()),
                MarkFWS, NowChar, Text(" ".into())
            ] ),
            ( Word {
                left_padding: None,
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( FWS ) ),
            }, vec![
                NowStr,
                Text("abc".into()),
                MarkFWS, NowChar, Text(" ".into())
            ] )
        ];

        for &( ref word, ref expection) in words.iter() {
            let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                do_encode_word( word, &mut handle, None ).unwrap();
                mem::forget(handle);
            }
            assert_eq!(
                &encoder.trace[1..],
                &expection[..]
            );
        }
    }


}