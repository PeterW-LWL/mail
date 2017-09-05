//FIXME use Fnv?
use std::ascii::AsciiExt;

use ascii::{ AsciiChar, AsciiStr };

use error::*;
use grammar::{is_ctl, is_tspecial };
use codec::{ MailEncodable, MailEncoder };
use utils::{ FileMeta, HeaderTryFrom };



#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Disposition {
    kind: DispositionKind,
    file_meta: DispositionParameters
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
struct DispositionParameters(FileMeta);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum DispositionKind {
    Inline, Attachment
}


impl Disposition {

    pub fn inline() -> Self {
        Disposition::new( DispositionKind::Inline, FileMeta::default() )
    }

    pub fn attachment() -> Self {
        Disposition::new( DispositionKind::Attachment, FileMeta::default() )
    }

    pub fn new( kind: DispositionKind, file_meta: FileMeta ) -> Self {
        Disposition { kind, file_meta: DispositionParameters( file_meta ) }
    }

    pub fn kind( &self ) -> DispositionKind {
        self.kind
    }

    pub fn file_meta( &self ) -> &FileMeta {
        &self.file_meta
    }

    pub fn file_meta_mut( &mut self ) -> &mut FileMeta {
        &mut self.file_meta
    }

}

/// This try from is for usability only, it is
/// generally recommendet to use Disposition::inline()/::attachment()
/// as it is type safe / compiler time checked, while this one
/// isn't
impl<'a> HeaderTryFrom<&'a str> for Disposition {
    fn try_from(text: &'a str) -> Result<Self> {
        if text.eq_ignore_ascii_case( "Inline" ) {
            Ok( Disposition::inline() )
        } else if text.eq_ignore_ascii_case( "Attachment" ) {
            Ok( Disposition::attachment() )
        } else {
            bail!( "content disposition can either be Inline or Attachment nothing else" )
        }
    }
}

macro_rules! encode_disposition_param {
    ( do $ec:expr, ( $($ch:ident)* ) $inner:ident | $value:expr => $code:block ) => ({
        if let Some( ref $inner ) = $value {
            $ec.write_char( AsciiChar::Semicolon );
            $ec.write_str( ascii_str!{ $($ch)* } );
            $ec.write_char( AsciiChar::Equal );
            $code
        }
    });

    ( $ec:expr, $( $tp:tt ( $($ch:ident)* )  $value:expr; )* ) => ({
        let encoder = $ec;
        $(
            encode_disposition_param!{ $tp encoder, ( $($ch)* ) $value }
        )*
    });

    ( STR $ec:expr,  ( $($ch:ident)* )  $value:expr ) => (
        encode_disposition_param!{ do $ec, ( $($ch)*) filename | $value => {
            encode_file_name( &**filename, $ec )?;
        }}
    );
    ( DATE $ec:expr, ( $($ch:ident)* ) $value:expr ) => (
        encode_disposition_param!{ do $ec, ( $($ch)* ) date | $value  => {
            $ec.write_char( AsciiChar::Quotation );
            date.encode( $ec )?;
            $ec.write_char( AsciiChar::Quotation );
        }}
    );
    ( USIZE $ec:expr,  ( $($ch:ident)* ) $value:expr ) => (
        encode_disposition_param!{ do $ec, ( $($ch)* ) val | $value  => {
            let val: usize = *val;
            //SAFETY: the string produced from converting a number to a (decimal) string is
            //  always ascii, as such it is always safe
            $ec.write_str( unsafe { AsciiStr::from_ascii_unchecked( &*val.to_string() ) } );
        }}
    );
}

//TODO provide a gnneral way for encoding header parameter ...
//  which follow the scheme: <mainvalue> *(";" <key>"="<value> )
//  this are: ContentType and ContentDisposition for now
impl<E> MailEncodable<E> for DispositionParameters where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        encode_disposition_param! {
            encoder,
            STR ( f i l e n a m e )  self.file_name;
            DATE ( c r e a t i o n Minus d a t e ) self.creation_date;
            DATE ( m o d i f i c a t i o n Minus d a t e ) self.modification_date;
            DATE ( r e a d Minus d a t e ) self.read_date;
            USIZE ( s i z e ) self.size;
        }
        Ok( () )
    }
}

impl<E> MailEncodable<E> for Disposition where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        use self::DispositionKind::*;
        match self.kind {
            Inline => {
                encoder.write_str( ascii_str!{ i n l i n e } );
            },
            Attachment => {
                encoder.write_str( ascii_str!{ a t t a c h m e n t } );
            }
        }
        self.file_meta.encode( encoder )?;
        Ok( () )
    }
}


fn encode_file_name<E>(file_name: &AsciiStr, encoder: &mut E) -> Result<()>
    where E: MailEncoder
{
    for achar in file_name.chars() {
        //TODO this needs way better handling
        let char = achar.as_char();
        if !char.is_ascii() ||  is_tspecial( char ) || is_ctl( char ) || char == ' '  {
            bail!(
                "handling non token file names in ContentDisposition is currently not supported" );
        } else {
            encoder.write_char( *achar );
        }
    }
    Ok( () )
}


deref0!{+mut DispositionParameters => FileMeta }

#[cfg(test)]
mod test {
    use ascii::IntoAsciiString;

    use super::*;
    use codec::test_utils::*;
    use components::DateTime;

    ec_test!{ no_params_inline, {
        Some( Disposition::inline() )
    } => ascii => [
        LinePart("inline")
    ]}

    ec_test!{ no_params_attachment, {
        Some( Disposition::attachment() )
    } => ascii => [
        LinePart("attachment")
    ]}

    ec_test!{ attachment_all_params, {
        Some( Disposition::new( DispositionKind::Attachment, FileMeta {
            file_name: Some( "random.png".into_ascii_string().unwrap() ),
            creation_date: Some( DateTime::test_time( 1 ) ),
            modification_date: Some( DateTime::test_time( 2 ) ),
            read_date: Some( DateTime::test_time( 3 ) ),
            size: Some( 4096 )
        }) )
    } => ascii => [
        LinePart( concat!( "attachment",
            ";filename=random.png",
            ";creation-date=\"Tue,  6 Aug 2013 04:11:01 +0000\"",
            ";modification-date=\"Tue,  6 Aug 2013 04:11:02 +0000\"",
            ";read-date=\"Tue,  6 Aug 2013 04:11:03 +0000\"",
            ";size=4096" ) )
    ]}

    //TODO: (1 allow FWS or so in parameters) (2 utf8 file names)

    #[test]
    fn test_from_str() {
        assert_ok!( Disposition::try_from( "Inline" ) );
        assert_ok!( Disposition::try_from( "InLine" ) );
        assert_ok!( Disposition::try_from( "Attachment" ) );

        assert_err!( Disposition::try_from( "In line") );
    }

}