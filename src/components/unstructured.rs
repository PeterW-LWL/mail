use error::*;
use codec::{ MailEncoder, MailEncodable };
use char_validators::{ is_vchar, is_ws, MailType };
use char_validators::encoded_word::EncodedWordContext;
use super::utils::text_partition::{partition, Partition};
use super::utils::item::Input;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unstructured {
    //FEATUR_TODO(non_utf8_input): split into parts each possibke having their own encoding
    text: Input,
}

impl Unstructured {
    pub fn from_input( text: Input ) -> Self {
        Unstructured { text }
    }

    pub fn from_string<I>( string: I ) -> Self
        where I: Into<String>
    {
        let string: String = string.into();

        Unstructured {
            text: Input::from( string )
        }
    }

}

impl MailEncodable for Unstructured {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
        let text: &str = &*self.text;
        if text.len() == 0 {
            return Ok( () )
        }
        // unstructured    =   (*([FWS] VCHAR) *WSP)
        // FWS             =   ([*WSP CRLF] 1*WSP)
        // + encoded words

        //1. split in sequence like FWS 1*VCHAR FWS 1*VCHAR ...
        //    use nom to parse and have some alternating struc,
        //    (e.g. (Vec<FWSBlock>, Vec<VCHARBlock>, started_with_FWS?)),
        //2. write FWS's possible write VCHAR blocks as encoded word/words
        //    - only at this point check for utf8, as in a Ascii we can
        //      encode utf8
        //    - also check for "malformed" FWS containing e.g. orphan '\n' or '\t'
        //      for not encode them  but later have some "strictness" level
        //      deceiding weither to 1. drop them , 2. error on them


        let blocks = partition( text )?;

        let mut biter = blocks.into_iter();

        //unwrap is safe because we pushed at last one (current_block)
        let this_block = biter.next().unwrap();
        for next_block in biter {
            match this_block {
                Partition::VCHAR( data ) => {
                    let needs_encoding = data
                        .chars()
                        .any(|ch| !is_vchar( ch, encoder.mail_type() ) );

                    if needs_encoding {
                        encoder.write_encoded_word( data, EncodedWordContext::Text )
                    } else {
                        // if needs_encoding is false all chars a vchars wrt. the mail
                        // type, therefore if the mail type is Ascii this can only be
                        // Ascii. Note that even writing Utf8 to a Ascii mail is safe
                        // wrt. rust, but incorrect nevertheless.
                        encoder.write_str_unchecked( data )
                    }
                },
                Partition::SPACE( data ) => {
                    //NOTE: space has to be at last one horizontal-white-space
                    // (required by the possibility of VCHAR partitions beeing
                    //  encoded words)

                    //let data = text[start..end];
                    //FIXME it currently collapses all FWS into a single space, possible folding
                    // if the line would be to long otherwise, this has the benefit that there
                    // won't be any "illegal" sequences like "\r \n"
                    encoder.write_fws()
                }
            }

        }




        //Note: the rfc 2047 does not directly state all use-cases of "unstructured" can be encoded
        // with encoded word's, but it list practically all cases unstructured can appear in
        //FIXME can contain encoded-word
        //TODO allow the data to contains thinks like '\t' etc.
        //FIXME do not replace any kind of whitespace with space

        Ok( () )
    }
}
