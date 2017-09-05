use ascii::AsciiChar;

use error::*;
use codec::{ MailEncoder, MailEncodable };
use utils::{ Vec1, HeaderTryFrom, HeaderTryInto };

use super::Phrase;


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PhraseList(pub Vec1<Phrase>);


impl<E> MailEncodable<E> for PhraseList where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
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

impl<T> HeaderTryFrom<T> for PhraseList
    where T: HeaderTryInto<Phrase>
{
    fn try_from( phrase: T ) -> Result<Self> {
        let phrase = phrase.try_into()?;
        Ok( PhraseList( Vec1::new( phrase ) ) )
    }
}


impl<T> HeaderTryFrom<Vec<T>> for PhraseList
    where T: HeaderTryInto<Phrase>
{
    fn try_from(vec: Vec<T>) -> Result<Self> {
        try_from_into_iter( vec )
    }
}

fn try_from_into_iter<IT>( phrases: IT ) -> Result<PhraseList>
    where IT: IntoIterator, IT::Item: HeaderTryInto<Phrase>
{
    let mut iter = phrases.into_iter();
    let mut vec = if let Some( first) = iter.next() {
        Vec1::new( first.try_into()? )
    } else {
        bail!( "header needs at last one mailbox" );
    };
    for phrase in iter {
        vec.push( phrase.try_into()? );
    }
    Ok( PhraseList( vec ) )
}

//FIXME: dedup code duplication with:
// MailboxList, PhraseList(this think here) and ?? possible future types??
macro_rules! impl_header_try_from_array {
    (_MBoxList 0) => ();
    (_MBoxList $len:tt) => (
        impl<T> HeaderTryFrom<[T; $len]> for PhraseList
            where T: HeaderTryInto<Phrase>
        {
            fn try_from( vec: [T; $len] ) -> Result<Self> {
                //due to only supporting arrays halfheartedly for now
                let heapified: Box<[T]> = Box::new(vec);
                let vecified: Vec<_> = heapified.into();
                try_from_into_iter( vecified )
            }
        }
    );
    ($($len:tt)*) => ($(
        impl_header_try_from_array!{ _MBoxList $len }
    )*);
}

impl_header_try_from_array! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
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
        //TODO really no FWS by the seperator??
        // (currently it's this way as word can start with a FWS making it a duble FWS)
        LinePart( "there, magic" ),
        FWS,
        LinePart( "man" )
    ]}

    ec_test!{ some_simple_phrases_try_from, {
        PhraseList::try_from(
            "hy there"
        )
    } => ascii => [
        LinePart( "hy" ),
        FWS,
        LinePart( "there" )
    ]}

    ec_test!{ some_phrases_try_from, {
        PhraseList::try_from( [
            "hy there",
            "magic man"
        ] )
    } => ascii => [
        LinePart( "hy" ),
        FWS,
        LinePart( "there, magic" ),
        FWS,
        LinePart( "man" )
    ]}
}

