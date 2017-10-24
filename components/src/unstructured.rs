use std::ops::{ Deref, DerefMut};

use soft_ascii_string::SoftAsciiChar;

use core::error::*;
use core::grammar::is_vchar;
use core::codec::{EncodeHandle, EncodableInHeader, EncodedWordEncoding, WriterWrapper};
use core::data::Input;
use core::utils::{HeaderTryInto, HeaderTryFrom};

use error::ComponentError::WSPOnlyPhrase;

use super::utils::text_partition::{partition, Partition};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unstructured {
    //FEATUR_TODO(non_utf8_input): split into parts each possibke having their own encoding
    text: Input,
}

impl Deref for Unstructured {
    type Target = Input;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl DerefMut for Unstructured {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.text
    }
}

impl<T> HeaderTryFrom<T> for Unstructured
    where T: HeaderTryInto<Input>
{
    fn try_from(text: T) -> Result<Self> {
        let text = text.try_into()?;
        Ok( Unstructured { text })
    }
}



impl EncodableInHeader for  Unstructured {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        let text: &str = &*self.text;
        if text.len() == 0 {
            return Ok( () )
        }

        let blocks = partition( text )?;

        let mut had_word = false;
        for block in blocks.into_iter() {
            match block {
                Partition::VCHAR( data ) => {
                    had_word = true;
                    let mail_type = handle.mail_type();
                    handle.write_if(data, |s|
                        s.chars().all(|ch| is_vchar(ch, mail_type))
                    ).handle_condition_failure(|handle| {
                        let encoding = EncodedWordEncoding::QuotedPrintable;
                        let mut writer = WriterWrapper::new(
                            encoding,
                            handle
                        );
                        encoding.encode(data, &mut writer);
                        Ok(())
                    })?;
                },
                Partition::SPACE( data ) => {
                    //NOTE: the usage of write_fws is relevant for braking the line and CRLF
                    // is still semantically ignored BUT, ther cant be any comments here,
                    // as we are in a unstructured header field
                    let mut had_fws = false;
                    for ch in data.chars() {
                        if ch == '\r' || ch == '\n' {
                            continue;
                        } else if !had_fws {
                            handle.mark_fws_pos();
                            had_fws = true;
                        }
                        handle.write_char( SoftAsciiChar::from_char_unchecked(ch) )?;
                    }
                    if !had_fws {
                        //currently this can only happen if data only consists of '\r','\n'
                        //NOTE: space has to be at last one horizontal-white-space
                        // (required by the possibility of VCHAR partitions beeing
                        //  encoded words)
                        handle.write_fws();
                    }
                }
            }

        }

        if had_word {
            Ok( () )
        } else {
            bail!(WSPOnlyPhrase);
        }

    }
}


#[cfg(test)]
mod test {
    use core::grammar::MailType;
    use core::codec::{Encoder, VecBodyBuf};

    use super::*;

    ec_test! { simple_encoding, {
        Unstructured::try_from( "this simple case" )?
    } => ascii => [
        Text "this",
        MarkFWS,
        Text " simple",
        MarkFWS,
        Text " case"
    ]}

    ec_test!{ simple_utf8,  {
         Unstructured::try_from( "thüs sümple case" )?
    } => utf8 => [
        Text "thüs",
        MarkFWS,
        Text " sümple",
        MarkFWS,
        Text " case"
    ]}

    ec_test!{ encoded_words,  {
         Unstructured::try_from( "↑ ↓ ←→ bA" )?
    } => ascii => [
        Text "=?utf8?Q?=E2=86=91?=",
        MarkFWS,
        Text " =?utf8?Q?=E2=86=93?=",
        MarkFWS,
        Text " =?utf8?Q?=E2=86=90=E2=86=92?=",
        MarkFWS,
        Text " bA"
    ]}

    ec_test!{ eats_cr_lf, {
        Unstructured::try_from( "a \rb\n c\r\n " )?
    } => ascii => [
        Text "a",
        MarkFWS,
        Text " b",
        MarkFWS,
        Text " c",
        MarkFWS,
        Text " "
    ]}

    ec_test!{ at_last_one_fws, {
        Unstructured::try_from( "a\rb\nc\r\n" )?
    } => ascii => [
        Text "a",
        MarkFWS,
        Text " b",
        MarkFWS,
        Text " c",
        MarkFWS,
        Text " "
    ]}

    ec_test!{ kinda_keeps_wsp, {
        Unstructured::try_from("\t\ta  b \t")?
    } => ascii => [
        MarkFWS,
        Text "\t\ta",
        MarkFWS,
        Text "  b",
        MarkFWS,
        Text " \t"
    ]}


    #[test]
    fn wsp_only_phrase_fails() {
        let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
        {
            let mut handle = encoder.encode_handle();
            let input = Unstructured::try_from( " \t " ).unwrap();
            assert_err!(input.encode( &mut handle ));
            handle.undo_header();
        }
    }
}