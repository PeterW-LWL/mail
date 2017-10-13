use std::ops::{ Deref, DerefMut};

use ascii::AsciiChar;

use error::*;
use grammar::is_vchar;
use grammar::encoded_word::EncodedWordContext;
use codec::{EncodeHeaderHandle, EncodableInHeader,EncodedWordEncoding};
use data::{ FromInput, EncodedWord };

use super::utils::text_partition::{partition, Partition};
use data::Input;

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

impl FromInput for Unstructured {
    fn from_input<I: Into<Input>>( text: I ) -> Result<Self> {
        Ok( Unstructured { text: text.into() } )
    }
}


impl EncodableInHeader for  Unstructured {

    fn encode(&self, handle: &mut EncodeHeaderHandle) -> Result<()> {
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
                    let needs_encoding = data
                        .chars()
                        .any(|ch| !is_vchar( ch, handle.mail_type() ) );

                    if needs_encoding {
                        EncodedWord::write_into( handle,
                                                 data,
                                                 EncodedWordEncoding::QuotedPrintable,
                                                 EncodedWordContext::Text );
                    } else {
                        // if needs_encoding is false all chars a vchars wrt. the mail
                        // type, therefore if the mail type is Ascii this can only be
                        // Ascii. Note that even writing Utf8 to a Ascii mail is safe
                        // wrt. rust, but incorrect nevertheless.
                        handle.write_str_unchecked( data )?;
                    }
                },
                Partition::SPACE( data ) => {
                    //NOTE: the usage of write_fws is relevant for braking the line and CRLF
                    // is still semantically ignored BUT, ther cant be any comments here,
                    // as we are in a unstructured header field
                    let mut had_fws = false;
                    for char in data.chars() {
                        if char == '\r' || char == '\n' {
                            continue;
                        } else if had_fws {
                            //OPTIMIZE: from_unchecked as char is always a char in this context
                            handle.write_char( AsciiChar::from( char ).unwrap() )?;
                        } else {
                            //FIXME allow writing fws based on '\t'
                            handle.write_fws();
                            had_fws = true;
                        }
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
            bail!( concat!( "can not encode WSP only phrase,",
                            "a phrase is required to contain at last one word" ) );
        }

    }
}


#[cfg(test)]
mod test {
    use grammar::MailType;
    use codec::{Encoder, VecBodyBuf};

    use super::*;

    ec_test! { simple_encoding, {
        Unstructured::from_input( "this simple case" )?
    } => ascii => [
        Text "this",
        MarkFWS,
        Text " simple",
        MarkFWS,
        Text " case"
    ]}

    ec_test!{ simple_utf8,  {
         Unstructured::from_input( "thüs sümple case" )?
    } => utf8 => [
        Text "thüs",
        MarkFWS,
        Text " sümple",
        MarkFWS,
        Text " case"
    ]}

    ec_test!{ encoded_words,  {
         Unstructured::from_input( "↑ ↓ ←→ bA" )?
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
        Unstructured::from_input( "a \rb\n c\r\n " )?
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
        Unstructured::from_input( "a\rb\nc\r\n" )?
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
        Unstructured::from_input("\t\ta  b \t")?
    } => ascii => [
        MarkFWS,
        Text " \ta",
        MarkFWS,
        Text "  b",
        MarkFWS,
        Text " \t"
    ]}


    #[test]
    fn wsp_only_phrase_fails() {
        let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
        {
            let mut handle = encoder.encode_header_handle();
            let input = Unstructured::from_input( " \t " ).unwrap();
            assert_err!(input.encode( &mut handle ));
            handle.undo_header();
        }
    }
}