use soft_ascii_string::SoftAsciiChar;

use error::*;
use codec::{self, EncodeHandle, EncodableInHeader };
use codec::idna;

use grammar::{
    is_ascii,
    is_atext,
    is_dtext,
    is_ws,
    MailType
};

use data::{FromInput, Input, SimpleItem, InnerUtf8 };


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

impl EncodableInHeader for  Email {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        self.local_part.encode( handle )?;
        handle.write_char( SoftAsciiChar::from_char_unchecked('@') )?;
        self.domain.encode( handle )?;
        Ok( () )
    }

}

impl FromInput for LocalPart {

    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        Ok( LocalPart( input.into() ) )
    }

}

impl EncodableInHeader  for LocalPart {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        let input: &str = &*self.0;
        let mail_type = handle.mail_type();

        let (got_mt, res) = codec::quoted_string::quote_if_needed(
            input,
            codec::quoted_string::DotAtomTextCheck::new(mail_type),
            mail_type
        )?;

        debug_assert!(!(got_mt == MailType::Internationalized && mail_type == MailType::Ascii));

        handle.mark_fws_pos();
        // if mail_type == Ascii quote_if_needed already made sure this
        // is ascii (or returned an error if not)
        // it also made sure it is valid as it is either `dot-atom-text` or `quoted-string`
        handle.write_str_unchecked(&*res)?;
        handle.mark_fws_pos();
        Ok( () )
    }
}



impl FromInput for Domain {
    fn from_input<I: Into<Input>>( input: I ) -> Result<Self> {
        let input = input.into();
        let item =
            match Domain::check_domain( &*input )? {
                MailType::Ascii | MailType::Mime8BitEnabled => {
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
    //SAFETY:
    //  the function is only allowed to return MailType::Ascii
    //  if the domain is actually ascii
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

impl EncodableInHeader for  Domain {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        handle.mark_fws_pos();
        match self.0 {
            SimpleItem::Ascii( ref ascii ) => {
                handle.write_str( ascii )?;
            },
            SimpleItem::Utf8( ref utf8 ) => {
                handle.write_if_utf8(utf8)
                    .handle_condition_failure(|handle| {
                        handle.write_str( &*idna::puny_code_domain( utf8 )? )
                    })?;
            }
        }
        handle.mark_fws_pos();
        Ok( () )
    }
}





#[cfg(test)]
mod test {
    use codec::{ Encoder, VecBodyBuf};
    use super::*;

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
        LocalPart::from_input(  "hans" )?
    } => ascii => [
        MarkFWS,
        Text "hans",
        MarkFWS
    ]}

    //fails tries to write utf8
    ec_test!{ local_part_quoted, {
        LocalPart::from_input(  "ha ns" )?
    } => ascii => [
        MarkFWS,
        Text "\"ha ns\"",
        MarkFWS
    ]}


    ec_test!{ local_part_utf8, {
        LocalPart::from_input( "Jörn" )?
    } => utf8 => [
        MarkFWS,
        Text "Jörn",
        MarkFWS
    ]}

    #[test]
    fn local_part_utf8_on_ascii() {
        let mut encoder = Encoder::<VecBodyBuf>::new( MailType::Ascii );
        let mut handle = encoder.encode_handle();
        let local = LocalPart::from_input( "Jörn" ).unwrap();
        assert_err!(local.encode( &mut handle ));
        handle.undo_header();
    }

    ec_test!{ domain, {
        Domain::from_input( "bad.at.domain" )?
    } => ascii => [
        MarkFWS,
        Text "bad.at.domain",
        MarkFWS
    ]}

    ec_test!{ domain_international, {
        Domain::from_input( "dömain" )?
    } => utf8 => [
        MarkFWS,
        Text "dömain",
        MarkFWS
    ]}


    ec_test!{ domain_encoded, {
        Domain::from_input( "dat.ü.dü" )?
    } => ascii => [
        MarkFWS,
        Text "dat.xn--tda.xn--d-eha",
        MarkFWS
    ]}


    ec_test!{ email_simple, {
        Email::from_input( "simple@and.ascii" )?
    } => ascii => [
        MarkFWS,
        Text "simple",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "and.ascii",
        MarkFWS
    ]}

}