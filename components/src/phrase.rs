use vec1::Vec1;
use core::error::*;
use core::grammar::encoded_word::EncodedWordContext;
use core::codec::{EncodableInHeader, EncodeHandle};
use core::utils::{HeaderTryFrom, HeaderTryInto};
use core::data::Input;

use super::utils::text_partition::{ Partition, partition };
use super::word::{ Word, do_encode_word };
use super::{ CFWS, FWS };



#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Phrase( pub Vec1<Word> );

impl Phrase {

    pub fn new<T: HeaderTryInto<Input>>(input: T) -> Result<Self> {
        //TODO isn't this 90% the same code as used by Unstructured
        //TODO it would make much more sense if Input::shared could be taken advantage of
        let input = input.try_into()?;

        //OPTIMIZE: words => shared, then turn partition into shares, too
        let mut last_gap = None;
        let mut words = Vec::new();
        for partition in partition( input.as_str() )?.into_iter() {
            match partition {
                Partition::VCHAR( word ) => {
                    let mut word = Word::try_from( word )?;
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

impl<'a> HeaderTryFrom<&'a str> for Phrase {
    fn try_from( input: &'a str) -> Result<Self> {
        Phrase::new(input)
    }
}

impl HeaderTryFrom<String> for Phrase {
    fn try_from( input: String) -> Result<Self> {
        Phrase::new(input)
    }
}

impl HeaderTryFrom<Input> for Phrase {
    fn try_from( input: Input) -> Result<Self> {
        Phrase::new(input)
    }
}



impl EncodableInHeader for  Phrase {

    //FEATURE_TODO(warn_on_bad_phrase): warn if the phrase contains chars it should not
    //  but can contain due to encoding, e.g. ascii CTL's
    fn encode(&self, heandle: &mut EncodeHandle) -> Result<()> {
        for word in self.0.iter() {
            do_encode_word( &*word, heandle, Some( EncodedWordContext::Phrase ) )?;
        }

        Ok( () )

    }
}

#[cfg(test)]
mod test {
    use core::utils::HeaderTryFrom;
    use super::Phrase;

    ec_test!{ simple, {
        Phrase::try_from("simple think")?
    } => ascii => [
        Text "simple",
        MarkFWS,
        Text " think"
    ]}

    ec_test!{ with_encoding, {
        Phrase::try_from(" hm nääds encoding")?
    } => ascii => [
        MarkFWS,
        Text " hm",
        MarkFWS,
        Text " =?utf8?Q?n=C3=A4=C3=A4ds?=",
        MarkFWS,
        Text " encoding"
    ]}
}



