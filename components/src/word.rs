use soft_ascii_string::SoftAsciiStr;

use core::error::*;
use core::codec::{
    EncodedWordEncoding,
    EncodableInHeader, EncodeHandle,
    WriterWrapper
};
use core::grammar::{MailType, is_atext};
use core::grammar::encoded_word::EncodedWordContext;
use core::codec::quoted_string;
use core::utils::{HeaderTryFrom, HeaderTryInto};
use core::data::Input;

use error::ComponentError::InvalidWord;


use super::CFWS;



#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Word {
    pub left_padding: Option<CFWS>,
    pub input: Input,
    pub right_padding: Option<CFWS>
}

impl<T> HeaderTryFrom<T> for Word
    where T: HeaderTryInto<Input>
{

    fn try_from( input: T ) -> Result<Self> {
        //TODO there should be a better way, I think I take the grammar to literal here
        // could not any WSP be a potential FWSP, do we really need this kind of fine gained
        // control, it feels kind of useless??
        let input = input.try_into()?;
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
    handle: &'a mut EncodeHandle<'b>,
    ecw_ctx: Option<EncodedWordContext>,
) -> Result<()> {

    if let Some( pad ) = word.left_padding.as_ref() {
        pad.encode( handle )?;
    }

    let input: &str = &*word.input;
    let mail_type = handle.mail_type();
    handle.write_if(input, |input| {
        (!input.starts_with("=?"))
            && input.chars().all( |ch| is_atext( ch, mail_type ) )

    }).handle_condition_failure(|handle| {
        if let Some( _ecw_ctx ) = ecw_ctx {
            //FIXME actually use the EncodedWordContext
            let encoding = EncodedWordEncoding::QuotedPrintable;
            let mut writer = WriterWrapper::new(
                encoding,
                handle
            );
            encoding.encode(input, &mut writer);
            Ok(())
        } else {
            let (target_mail_type, quoted) = quoted_string::quote(input)?;
            match target_mail_type {
                MailType::Ascii | MailType::Mime8BitEnabled => {
                    handle.write_str(SoftAsciiStr::from_str_unchecked(quoted.as_str()))
                },
                MailType::Internationalized => {
                    if handle.mail_type().is_internationalized() {
                        handle.write_utf8(quoted.as_str())
                    } else {
                        bail!(InvalidWord(input.to_owned()))
                    }
                }
            }
        }
    })?;

    if let Some( pad ) = word.right_padding.as_ref() {
        pad.encode( handle )?;
    }
    Ok( () )
}


#[cfg(test)]
mod test {
    use std::mem;

    use core::grammar::MailType;
    use core::codec::{ Encoder, VecBodyBuf, EncodableClosure};
    use core::codec::TraceToken::*;
    use core::codec::simplify_trace_tokens;

    use super::*;
    use super::super::FWS;

    ec_test!{encode_pseudo_encoded_words, {
        let word = Word::try_from( "=?" )?;
        EncodableClosure(move |handle: &mut EncodeHandle| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        Text "=?utf8?Q?=3D=3F?="
    ]}

    ec_test!{encode_word, {
        let word = Word::try_from( "a↑b" )?;
        EncodableClosure(move |handle: &mut EncodeHandle| {
            do_encode_word( &word, handle, Some( EncodedWordContext::Text ) )
        })
    } => ascii => [
        Text "=?utf8?Q?a=E2=86=91b?="
    ]}


    #[test]
    fn encode_fails() {
        let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
        let mut handle = encoder.encode_handle();
        let word = Word::try_from( "a↑b" ).unwrap();
        assert_err!(do_encode_word( &word, &mut handle, None ));
        handle.undo_header();
    }


    ec_test!{quoted_fallback, {
        let word = Word::try_from( "a\"b" )?;
        EncodableClosure(move |handle: &mut EncodeHandle| {
            do_encode_word( &word, handle, None )
        })
    } => ascii => [
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
                Text("abc".into())
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( FWS) ),
                input: "abc".into(),
                right_padding: None,
            }, vec![
                MarkFWS,
                Text(" abc".into())
            ] ),
            ( Word {
                left_padding: Some( CFWS::SingleFws( FWS ) ),
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( FWS ) ),
            }, vec![
                MarkFWS,
                Text(" abc".into()),
                MarkFWS,
                Text(" ".into())
            ] ),
            ( Word {
                left_padding: None,
                input: "abc".into(),
                right_padding: Some( CFWS::SingleFws( FWS ) ),
            }, vec![
                Text("abc".into()),
                MarkFWS,
                Text(" ".into())
            ] )
        ];

        for &( ref word, ref expection) in words.iter() {
            let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                do_encode_word( word, &mut handle, None ).unwrap();
                mem::forget(handle);
            }
            assert_eq!(
                &simplify_trace_tokens(encoder.trace.into_iter().skip(1)),
                expection
            );
        }
    }


}