use error::*;
use types::Vec1;
use codec::{ MailEncodable, MailEncoder };
use char_validators::encoded_word::EncodedWordContext;

use super::utils::item::{ Input, Item };
use super::utils::text_partition::{ Partition, partition };
use super::word::{ Word, do_encode_word };
use super::{ CFWS, FWS };

//FIXME PhraseWord's <==> Word's, just some other Word usage is more restricted we don't need this
// mainly the other usage does not allow EncodedWords, but since the changes to Item this can
// be checked at encoding time! Yay!
#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct PhraseWord( Word );

impl PhraseWord {
    pub fn new( item: Item ) -> Result<Self> {
        Ok( PhraseWord( Word::new( item, true )? ) )
    }

    pub fn from_parts(
        left_padding: Option<CFWS>,
        item: Item,
        right_padding: Option<CFWS>,
    ) -> Result<Self> {
        Ok( PhraseWord( Word::from_parts( left_padding, item, right_padding, true )? ) )
    }

}

deref0!{ +mut PhraseWord => Word }

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
                    let word_item = Item::Input( Input::Owned( word.into() ) );
                    //FIXME change Word::from_parts
                    let word = Word::from_parts( last_gap.take(), word_item, None, true )?;
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



