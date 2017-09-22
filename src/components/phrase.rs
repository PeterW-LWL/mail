use error::*;
use external::vec1::Vec1;
use codec::{ MailEncodable, MailEncoder };
use grammar::encoded_word::EncodedWordContext;

use data::{ FromInput, Input };
use super::utils::text_partition::{ Partition, partition };
use super::word::{ Word, do_encode_word };
use super::{ CFWS, FWS };



#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Phrase( pub Vec1<Word> );

impl FromInput for Phrase {
    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        //TODO isn't this 90% the same code as used by Unstructured
        let input = input.into();

        //OPTIMIZE: words => shared, then turn partition into shares, too
        let mut last_gap = None;
        let mut words = Vec::new();
        for partition in partition( &*input )?.into_iter() {
            match partition {
                Partition::VCHAR( word ) => {
                    let mut word = Word::from_input( word )?;
                    if let Some( fws ) = last_gap.take() {
                        word.pad_left( fws );
                    }
                    words.push( word );
                },
                Partition::SPACE( _gap ) => {
                    //FIMXE currently collapses WS
                    last_gap = Some( CFWS::SingleFws( FWS ) )
                }
            }
        }

        let mut words = Vec1::from_vec( words )
            .map_err( |_|-> Error { "a phras has to have at last one word".into() } )?;

        if let Some( right_padding ) = last_gap {
            words.last_mut().pad_right( right_padding );
        }

        Ok( Phrase( words ) )
    }
}


impl<E> MailEncodable<E> for Phrase where E: MailEncoder {

    //FEATURE_TODO(warn_on_bad_phrase): warn if the phrase contains chars it should not
    //  but can contain due to encoding, e.g. ascii CTL's
    fn encode(&self, encoder: &mut E) -> Result<()> {
        for word in self.0.iter() {
            do_encode_word( &*word, encoder, Some( EncodedWordContext::Phrase ) )?;
        }

        Ok( () )

    }
}

#[cfg(test)]
mod test {
    use data::FromInput;
    use codec::test_utils::*;
    use super::Phrase;

    ec_test!{ simple, {
        Phrase::from_input("simple think")
    } => ascii => [
        LinePart("simple"),
        FWS,
        LinePart("think")
    ]}

    ec_test!{ with_encoding, {
        Phrase::from_input(" hm nääds encoding")
    } => ascii => [
        FWS,
        LinePart("hm"),
        FWS,
        LinePart( "=?utf8?Q?n=C3=A4=C3=A4ds?=" ),
        FWS,
        LinePart( "encoding" )
    ]}
}



