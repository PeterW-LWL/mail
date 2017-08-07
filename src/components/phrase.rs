use error::*;
use types::Vec1;
use codec::{ MailEncodable, MailEncoder };
use grammar::encoded_word::EncodedWordContext;

use data::{ FromInput, Input };
use super::utils::text_partition::{ Partition, partition };
use super::word::{ Word, do_encode_word };
use super::{ CFWS, FWS };



#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Phrase( pub Vec1<Word> );

//// while it is possible to store a single string,
//// it is not future prove as some words can be given
//// in a different encoding then the rest...
//
//pub enum Phrase {
//    //TODO consider only using Vec1<Word> and converting from
//    // Input if needed, we can put every "word" input eith
//    // the Ascii, Encoded or Utf8 box and if we have a
//    // Ascii Mail type + Utf8 we can just write an encoded word!
//    // Problem: Quoted encoding can be Ascii AND/OR Utf8
//    //
//    // also if we have e.g. Quoted Utf8 on Ascii mail we would have
//    // to "unquote" it and then use encoded word, that succs, but
//    // if we don't use Quoted Encoded Words with utf8 as item it's
//    // fine and the unquote=>encode_word think will become a "feature"
//    ItemBased( Vec1<Word> ),
//    InputBased( Input )
//}


impl Phrase {

    pub fn from_words( words: Vec1<Word> ) -> Self {
        Phrase( words )
    }

    pub fn from_input( input: Input ) -> Result<Self> {
        //OPTIMIZE: words => shared, then turn partition into shares, too
        let mut last_gap = None;
        let mut words = Vec::new();
        for partition in partition( &*input )?.into_iter() {
            match partition {
                Partition::VCHAR( word ) => {
                    let mut word = Word::from_input( word.into() )?;
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



impl MailEncodable for Phrase  {

    //FEATURE_TODO(warn_on_bad_phrase): warn if the phrase contains chars it should not
    //  but can contain due to encoding, e.g. ascii CTL's
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {

        for word in self.0.iter() {
            do_encode_word( &*word, encoder, Some( EncodedWordContext::Phrase ) )?;
        }

        Ok( () )

    }
}



