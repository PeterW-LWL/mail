//FIXME use Fnv?
use std::ascii::AsciiExt;

use ascii::{ AsciiChar, AsciiStr };

use error::*;
use codec::{ MailEncodable, MailEncoder };
use char_validators::{ is_ctl, is_tspecial };
use types::FileMeta;



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
impl MailEncodable for DispositionParameters {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
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

impl MailEncodable for Disposition {
    fn encode<E>(&self, encoder: &mut E) -> Result<()>
        where E: MailEncoder
    {
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
