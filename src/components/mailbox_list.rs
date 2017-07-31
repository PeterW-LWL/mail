

use error::*;
use ascii::AsciiChar;
use codec::{MailEncoder, MailEncodable };

use types::Vec1;
use super::Mailbox;


pub struct OptMailboxList( pub Vec<Mailbox> );
pub struct MailboxList( pub Vec1<Mailbox> );

impl MailboxList {
    pub fn from_single( m: Mailbox ) -> Self {
        MailboxList( Vec1::new( m ) )
    }
}


impl MailEncodable for OptMailboxList {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
       encode_list( self.0.iter(), encoder )
    }
}

impl MailEncodable for MailboxList {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        encode_list( self.0.iter(), encoder )
    }
}

fn encode_list<'a, E, I>( list_iter: I, encoder: &mut E ) -> Result<()>
    where E: MailEncoder,
          I: Iterator<Item=&'a Mailbox>
{
    sep_for!{ mailbox in list_iter;
        sep {
            encoder.write_char( AsciiChar::Comma );
            encoder.write_fws();
        };
        mailbox.encode( encoder )?;
    }
    Ok( () )
}

deref0!{ +mut OptMailboxList => Vec<Mailbox> }
deref0!{ +mut MailboxList => Vec<Mailbox> }
//
//#[cfg(test)]
//mod test {
//    use super::*;
//
//    mod encode {
//        use super::*;
//
//        fn parse( s: &str ) -> Address {
//            unimplemented!()
//        }
//
//        macro_rules! test {
//            ($name:ident, [$($addr:expr),*], $output:expr) => {
//                #[test]
//                fn $name() {
//                    let list = AddressList::new( vec![ $($addr),* ] ).unwrap();
//                    let mut encoder = MailEncoder::new( false );
//                    list.encode( &mut encoder ).expect( "encoding failed" );
//                    let encoded_bytes: Vec<_> = encoder.into();
//                    assert_eq!( $output, String::from_utf8_lossy( &*encoded_bytes ) );
//                }
//            }
//        }
//
//        //FIXME empty should err
////        test!{ empty,
////            [], "" }
//
//        test!{ one,
//            [ parse( "X <a@b.d>" ) ],
//            "X <a@b.d>" }
//
//        test!{ multiple,
//            [ parse( "X <a@b.d>" ), parse( "e@d.e" ), parse( "xe@de.de" ) ],
//            "X <a@b.d>, e@d.e, xe@de.de" }
//
//
//    }
//}