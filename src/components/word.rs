use error::*;
use codec::{ MailEncodable, MailEncoder };

use super::utils::item::Item;

use char_validators::{
    is_atext, is_ctl, MailType
};
use char_validators::quoted_word::is_quoted_word;
use char_validators::encoded_word::{
    is_encoded_word,
    EncodedWordContext
};
use codec::quote::{ unquote, quote, Quoted };

use super::CFWS;


#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Word(Option<CFWS>, Item, Option<CFWS> );


impl Word {
    pub fn check_item_validity(item: &Item, allow_encoded_word: bool) -> Result<()> {
        match *item {
            Item::Input( ref input ) => {
                if input.chars().any( is_ctl ) {
                    bail!( "a word can never contain CTL characters" );
                }
            },

            Item::QuotedString( ref quoted ) => {
                // Quoted already (should) do the checks, but does not for now
                if !is_quoted_word( &**quoted, MailType::Internationalized ) {
                    bail!( "invalide quoted word" );
                }
            },

            Item::EncodedWord( ref inner_ascii ) => {
                //FIXME we might want to place a EncodedWord Type there
                if !(allow_encoded_word &&
                        is_encoded_word( inner_ascii.as_str(), EncodedWordContext::Phrase ) )
                {
                    bail!( "invalide encoded word" );
                }
            }
        }

        Ok( () )
    }

    pub fn new( item: Item, allow_encoded_word: bool ) -> Result<Self> {
        Self::check_item_validity( &item, allow_encoded_word )?;
        Ok( Word( None, item, None ) )
    }

    pub fn from_parts(
        left_padding: Option<CFWS>,
        item: Item,
        right_padding: Option<CFWS>,
        allow_encoded_word: bool
    ) -> Result<Self> {

        Self::check_item_validity( &item, allow_encoded_word )?;
        Ok( Word( left_padding, item, right_padding ) )
    }

    pub fn pad_left( &mut self, padding: CFWS) {
        self.0 = Some( padding )
    }

    pub fn pad_right( &mut self, padding: CFWS) {
        self.2 = Some( padding )
    }

}

/// != encoded-word, through it might create an encoded-word
pub fn do_encode_word<E: MailEncoder>(
    word: &Word,
    encoder: &mut E,
    ecw_ctx: Option<EncodedWordContext>,
) -> Result<()> {
    use self::Item::*;
    if let Some( pad ) = word.0.as_ref() {
        pad.encode( encoder )?;
    }
    match word.1 {
        Input( ref word ) => {
            let mail_type = encoder.mail_type();
            let ok = (!word.starts_with("=?")) && word.chars()
                .all( |ch| is_atext( ch, mail_type ) );

            if ok {
                encoder.write_str_unchecked( &*word )
            } else {
                if let Some( ecw_ctx ) = ecw_ctx {
                    encoder.write_encoded_word( &*word, ecw_ctx )
                } else if let Ok( quoted ) = quote( &*word ) {
                    match quoted {
                        Quoted::Ascii( ref data ) => encoder.write_str( data ),
                        Quoted::Utf8( ref data ) => {
                            if encoder.try_write_utf8( data ).is_err() {
                                bail!( "can not write utf8 quoted string to ascii mail" );
                            }
                        }
                    }
                } else {
                    bail!( "can neither quote nor encode word: {:?}", &*word );
                }
            }
        },
        EncodedWord( ref ecw ) => {
            encoder.write_str( ecw );
        },
        QuotedString( ref quoted ) => {
            let status = match *quoted {
                Quoted::Ascii( ref astring ) => {
                    encoder.write_str( astring );
                    Ok( () )
                },
                Quoted::Utf8( ref string ) => encoder.try_write_utf8( string )
            };
            if status.is_err() {
                if let Some( ecw_ctx ) = ecw_ctx {
                    encoder.write_encoded_word( &*unquote( quoted ), ecw_ctx );
                } else {
                    bail!( "can not encode utf8 for this word, as encoded-words are not allowed" );
                }
            }
        }
    }
    if let Some( pad ) = word.2.as_ref() {
        pad.encode( encoder )?;
    }
    Ok( () )
}

