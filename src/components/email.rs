use ascii::{ AsciiChar };

use error::*;
use codec::{ MailEncoder, MailEncodable };
use codec::utf8_to_ascii::puny_code_domain;

use grammar::{
    is_ascii,
    is_atext,
    is_dtext,
    is_ws,
    is_quotable,
    MailType
};

use data::{FromInput, Input, QuotedString, SimpleItem, InnerUtf8 };


/// an email of the form `local-part@domain`
/// corresponds to RFC5322 addr-spec, so `<`, `>` padding is _not_
/// part of this Email type (but of the Mailbox type instead)
#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct Email {
    pub local_part: LocalPart,
    pub domain: Domain
}


#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct LocalPart( Input );


#[derive(Debug,  Clone, Hash, PartialEq, Eq)]
pub struct Domain( SimpleItem );


impl FromInput for Email {

    fn from_input<I: Into<Input>>( email: I ) -> Result<Self> {
        let email = email.into().into_shared();
        match email {
            Input( InnerUtf8::Owned( .. ) ) => unreachable!(),
            Input( InnerUtf8::Shared( shared ) ) => {
                //1. ownify Input
                //2. get 2 sub shares split befor/after @
                let index = shared.find( "@" )
                    .ok_or_else( ||-> Error { "invalide email".into() } )?; //bail!( "" )

                let left = shared.clone().map( |all| &all[..index] );
                let local_part = LocalPart::from_input( Input( InnerUtf8::Shared( left ) ) )?;
                //index+1 is ok as '@'.utf8_len() == 1
                let right = shared.map( |all| &all[index+1..] );
                let domain = Domain::from_input( Input( InnerUtf8::Shared( right ) ) )?;
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

impl FromInput for LocalPart {

    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        Ok( LocalPart( input.into() ) )
    }

}

impl MailEncodable for LocalPart {
    fn encode<E>( &self, encoder: &mut E ) -> Result<()>
        where E: MailEncoder
    {
        encoder.note_optional_fws();

        let input: &str = &*self.0;

        //OPTIMIZE: directly write to encoder
        //  REQUIREMENT: the encoder has to have something like encoder.resetable_writer(),
        //    which allows us to split a write_str/write_str_unchecked into multiple
        //    chunks, while allowing us to "abort" this write
        let mut requires_quoting = false;
        let mut mail_type = MailType::Ascii;
        for char in input.chars() {
            if !is_atext( char, mail_type ) {
                if !is_ascii( char ) {
                    mail_type = MailType::Internationalized;
                    if is_atext( char, mail_type ) {
                        continue;
                    }
                }
                if is_quotable( char ) {
                    requires_quoting = true;
                    // the quoting code will also iter over it so no need
                    // to continue here
                    break
                } else {
                    bail!( "unquotable charachter {:?} in local part", char );
                }
            }
        }

        if requires_quoting {
            QuotedString::write_into( encoder, input )?;
        } else {
            match mail_type {
                MailType::Ascii => encoder.write_str_unchecked( input ),
                MailType::Internationalized => encoder.try_write_utf8( input )?
            }
        }


        encoder.note_optional_fws();
        Ok( () )
    }
}



impl FromInput for Domain {
    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        let input = input.into();
        let item =
            match Domain::check_domain( &*input )? {
                MailType::Ascii => {
                    let asciied = unsafe { input.into_ascii_item_unchecked() };
                    SimpleItem::Ascii( asciied )
                },
                MailType::Internationalized => {
                    SimpleItem::from_utf8_input( input )
                }
            };

        Ok( Domain( item ) )
    }
}

impl Domain {
    fn check_domain( domain: &str ) -> Result<MailType> {
        let mut ascii = true;
        if domain.starts_with("[") && domain.ends_with("]") {
            //check domain-literal
            //for now the support of domain literals is limited i.e:
            //  1. no contained line
            //  2. no leading/trailing CFWS before/after the "["/"]"
            for char in domain.chars() {
                if ascii { ascii = is_ascii( char ) }
                if !( is_dtext( char, MailType::Internationalized) || is_ws( char ) ) {
                    bail!( "illigal domain-literal: {:?}", domain );
                }
            }
        } else {
            //check dot-atom-text
            // when supported Comments will be supported through the type system,
            // not stringly typing
            let mut dot_alowed = false;
            for char in domain.chars() {
                if ascii { ascii = is_ascii( char ) }
                if char == '.' && dot_alowed {
                    dot_alowed = false;
                } else if !is_atext( char, MailType::Internationalized ) {
                    bail!( "invalide domain name: {:?}", domain )
                } else {
                    dot_alowed = true;
                }
            }
        }
        Ok( if ascii {
            MailType::Ascii
        } else {
            MailType::Internationalized
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
    fn email_from_input() {
        let email = Email::from_input( "abc@de.fg" ).unwrap();
        assert_eq!(
            Email {
                local_part: LocalPart::from_input( "abc" ).unwrap(),
                domain: Domain::from_input( "de.fg" ).unwrap()
            },
            email
        )
    }



    ec_test!{ local_part_simple, {
        LocalPart::from_input(  "hans" )
    } => ascii => [
        OptFWS,
        LinePart("hans"),
        OptFWS
    ]}

    //fails tries to write utf8
    ec_test!{ local_part_quoted, {
        LocalPart::from_input(  "ha ns" )
    } => ascii => [
        OptFWS,
        LinePart("\"ha\\ ns\""),
        OptFWS
    ]}


    ec_test!{ local_part_utf8, {
        LocalPart::from_input( "Jörn" )
    } => utf8 => [
        OptFWS,
        LinePart( "Jörn" ),
        OptFWS
    ]}

    #[test]
    fn local_part_utf8_on_ascii() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        let local = LocalPart::from_input( "Jörn" ).unwrap();
        let res = local.encode( &mut ec );
        assert_eq!( false, res.is_ok() );
    }

    ec_test!{ domain, {
        Domain::from_input( "bad.at.domain" )
    } => ascii => [
        OptFWS,
        LinePart( "bad.at.domain" ),
        OptFWS
    ]}

    ec_test!{ domain_international, {
        Domain::from_input( "dömain" )
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
        Email::from_input( "simple@and.ascii" )
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