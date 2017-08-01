use ascii::{ AsciiChar,  IntoAsciiString };

use error::*;
use codec::{ MailEncoder, MailEncodable };
use codec::utf8_to_ascii::puny_code_domain;
use codec::quote::quote;
use char_validators::{ is_atext, MailType };


use super::utils::item::{ SimpleItem, Input, InnerAsciiItem, InnerUtf8Item };

/// an email of the form `local-part@domain`
/// corresponds to RFC5322 addr-spec, so `<`, `>` padding is _not_
/// part of this Email type (but of the Mailbox type instead)
#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct Email {
    pub local_part: LocalPart,
    pub domain: Domain
}


#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct LocalPart( SimpleItem );


#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct Domain( SimpleItem );


impl Email {

    pub fn from_input( email: Input ) -> Result<Self> {
        let email = email.into_shared();
        match email {
            Input::Owned( .. ) => unreachable!(),
            Input::Shared( shared ) => {
                //1. ownify Input
                //2. get 2 sub shares split befor/after @
                let index = shared.find( "@" )
                    .ok_or_else( ||-> Error { "invalide email".into() } )?; //bail!( "" )

                let left = shared.clone().map( |all| &all[..index] );
                let local_part = LocalPart::from_input( Input::Shared( left ) )?;
                //index+1 is ok as '@'.utf8_len() == 1
                let right = shared.map( |all| &all[index+1..] );
                let domain = Domain::from_input( Input::Shared( right ) );
                Ok( Email { local_part, domain } )
            }
        }



    }
}

impl MailEncodable for Email {

    fn encode<E>( &self, encoder: &mut E ) -> Result<()>
        where E: MailEncoder
    {
        self.local_part.encode( encoder )?;
        encoder.write_char( AsciiChar::At );
        self.domain.encode( encoder )?;
        Ok( () )
    }

}

impl LocalPart {

    pub fn from_input( input: Input ) -> Result<Self> {
        let mut requires_quoting = false;
        let mut mail_type = MailType::Ascii;
        for char in input.chars() {
            if !is_atext( char, mail_type ) {
                if char.len_utf8() > 1 {
                    mail_type = MailType::Internationalized;
                    if is_atext( char, mail_type ) {
                        continue;
                    }
                }
                requires_quoting = true;
            }
        }
        let input = if requires_quoting {
            Input::Owned( quote( &*input )?.into_string() )
        } else {
            input
        };

        Ok( LocalPart( match mail_type {
            MailType::Internationalized => SimpleItem::Utf8( input.into_utf8_item() ),
            MailType::Ascii => {
                //OPTIMIZE: it should be guaranteed to be ascii
                //SimpleItem::Ascii( unsafe { input.into_ascii_item_unchecked() } )
                SimpleItem::Ascii( input.into_ascii_item().unwrap() )
            }
        } ) )
    }
}

impl MailEncodable for LocalPart {
    fn encode<E>( &self, encoder: &mut E ) -> Result<()>
        where E: MailEncoder
    {
        use super::utils::item::SimpleItem::*;
        encoder.note_optional_fws();
        match self.0 {
            Ascii( ref ascii ) => {
                encoder.write_str( ascii );
            },
            Utf8( ref utf8 ) => {
                encoder.try_write_utf8( utf8 )?;
            }
        }
        encoder.note_optional_fws();
        Ok( () )
    }
}

impl Domain {
    pub fn from_input( inp: Input ) -> Self {
        let string = match inp {
            Input::Owned( string ) => string,
            Input::Shared( ref_to_string ) => String::from( &*ref_to_string ),
        };

        Domain( match string.into_ascii_string() {
            Ok( ascii ) => SimpleItem::Ascii( InnerAsciiItem::Owned( ascii ) ),
            Err( ascii_err ) => SimpleItem::Utf8( InnerUtf8Item::Owned( ascii_err.into_source() ) )
        } )
    }
}

impl MailEncodable for Domain {
    fn encode<E>( &self, encoder: &mut E ) -> Result<()>
        where E: MailEncoder
    {
        encoder.note_optional_fws();
        match self.0 {
            SimpleItem::Ascii( ref ascii ) => {
                encoder.write_str( ascii )
            },
            SimpleItem::Utf8( ref utf8 ) => {
                if encoder.try_write_utf8( utf8 ).is_err() {
                    puny_code_domain( utf8, encoder );
                }
            }
        }
        encoder.note_optional_fws();
        Ok( () )
    }
}





#[cfg(test)]
mod test {
    use super::*;
    use codec::test_utils::*;

    #[test]
    fn quote_simple() {
        assert_eq!( "\"tralala\"", &*quote("tralala").unwrap() );
    }

    #[test]
    fn quote_some_chars() {
        assert_eq!(  "\"tr@al\\ al\\\"a\"", &*quote("tr@al al\"a").unwrap() );
    }

    #[test]
    fn quote_ctl() {
        let res = quote("\x01");
        assert_eq!( false, res.is_ok() );
    }


    #[test]
    fn email_from_input() {
        let email = Email::from_input( "abc@de.fg".into() ).unwrap();
        assert_eq!(
            Email {
                local_part: LocalPart::from_input( "abc".into() ).unwrap(),
                domain: Domain::from_input( "de.fg".into() )
            },
            email
        )
    }



    ec_test!{ local_part_simple, {
        LocalPart::from_input(  "hans".into() ).unwrap()
    } => ascii => [
        OptFWS,
        LinePart("hans"),
        OptFWS
    ]}

    //fails tries to write utf8
    ec_test!{ local_part_quoted, {
        LocalPart::from_input(  "ha ns".into() ).unwrap()
    } => ascii => [
        OptFWS,
        LinePart("\"ha\\ ns\""),
        OptFWS
    ]}


    ec_test!{ local_part_utf8, {
        LocalPart::from_input( "Jörn".into() ).unwrap()
    } => utf8 => [
        OptFWS,
        LinePart( "Jörn" ),
        OptFWS
    ]}

    #[test]
    fn local_part_utf8_on_ascii() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        let local = LocalPart::from_input( "Jörn".into() ).unwrap();
        let res = local.encode( &mut ec );
        assert_eq!( false, res.is_ok() );
    }

    ec_test!{ domain, {
        Domain::from_input( "bad.at.domain".into() )
    } => ascii => [
        OptFWS,
        LinePart( "bad.at.domain" ),
        OptFWS
    ]}

    ec_test!{ domain_international, {
        Domain::from_input( "dömain".into() )
    } => utf8 => [
        OptFWS,
        LinePart( "dömain" ),
        OptFWS
    ]}

//TODO implement punnycode
//    ec_test!{ domain_encoded, {
//
//    } => ascii => [
//
//    ]}
//


    ec_test!{ email_simple, {
        Email {
            local_part: LocalPart::from_input( "simple".into() ).unwrap(),
            domain: Domain::from_input( "and.ascii".into() )
        }
    } => ascii => [
        OptFWS,
        LinePart( "simple" ),
        OptFWS,
        LinePart( "@" ),
        OptFWS,
        LinePart( "and.ascii" ),
        OptFWS
    ]}

}