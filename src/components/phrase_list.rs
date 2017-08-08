use ascii::AsciiChar;

use error::*;
use codec::{ MailEncoder, MailEncodable };
use types::Vec1;

use super::Phrase;


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PhraseList(pub Vec1<Phrase>);


impl MailEncodable for PhraseList {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        sep_for!{ word in self.0.iter();
            sep {
                //Note that we do not want to write FWS, as the following word might contains
                // a left_padding with a FWS, but a space if fine
                encoder.write_char( AsciiChar::Comma );
                encoder.write_char( AsciiChar::Space );
            };
            word.encode( encoder )?;

        }

        Ok( () )
    }
}

#[cfg(test)]
mod test {
    use data::FromInput;
    use codec::test_utils::*;
    use super::*;

    ec_test!{ some_phrases, {
        Some( PhraseList( vec1![
            Phrase::from_input( "hy there" ).unwrap(),
            Phrase::from_input( "magic man" ).unwrap()
        ]) )
    } => ascii => [
        LinePart( "hy" ),
        FWS,
        LinePart( "there, magic" ),
        FWS,
        LinePart( "man" )
    ]}
}

