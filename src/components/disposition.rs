use std::ascii::AsciiExt;

use error::*;
use codec::{EncodableInHeader, EncodeHeaderHandle};
use utils::{ FileMeta, HeaderTryFrom };
use components::mime::create_encoded_mime_parameter;


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


//TODO provide a gnneral way for encoding header parameter ...
//  which follow the scheme: <mainvalue> *(";" <key>"="<value> )
//  this are: ContentType and ContentDisposition for now
impl EncodableInHeader for DispositionParameters {

    fn encode(&self, handle: &mut EncodeHeaderHandle) -> Result<()> {
        let mt = handle.mail_type();
        let mut out = String::new();
        if let Some(filename) = self.file_name.as_ref() {
            create_encoded_mime_parameter(
                "filename", filename, &mut out, mt)?;
        }
        if let Some(creation_date) = self.creation_date.as_ref() {
            create_encoded_mime_parameter(
                "creation-date", creation_date.to_rfc2822(), &mut out, mt)?;
        }
        if let Some(date) = self.modification_date.as_ref() {
            create_encoded_mime_parameter(
                "modification-date", date.to_rfc2822(), &mut out, mt)?;
        }
        if let Some(date) = self.read_date.as_ref() {
            create_encoded_mime_parameter(
                "read-date", date.to_rfc2822(), &mut out, mt)?;
        }
        if let Some(size) = self.size.as_ref() {
            create_encoded_mime_parameter(
                "size", size.to_string(), &mut out, mt)?;
        }
        //TODO this function will be removed so do it differently
        handle.write_str_unchecked(&*out);
        Ok( () )
    }
}


impl EncodableInHeader for Disposition {

    fn encode(&self, handle: &mut EncodeHeaderHandle) -> Result<()> {
        use self::DispositionKind::*;
        match self.kind {
            Inline => {
                handle.write_str( ascii_str!{ i n l i n e } )?;
            },
            Attachment => {
                handle.write_str( ascii_str!{ a t t a c h m e n t } )?;
            }
        }
        self.file_meta.encode( handle )?;
        Ok( () )
    }
}


deref0!{+mut DispositionParameters => FileMeta }

#[cfg(test)]
mod test {
    use std::default::Default;

    use super::*;
    use components::DateTime;

    ec_test!{ no_params_inline, {
        Disposition::inline()
    } => ascii => [
        Text "inline"
    ]}

    ec_test!{ no_params_attachment, {
        Disposition::attachment()
    } => ascii => [
        Text "attachment"
    ]}

    ec_test!{ attachment_encode_file_name, {
        Disposition::new( DispositionKind::Attachment, FileMeta {
            file_name: Some("this is nice".to_owned()),
            ..Default::default()
        })
    } => ascii => [
        Text "attachment;filename=\"this is nice\""
    ]}

    ec_test!{ attachment_all_params, {
        Disposition::new( DispositionKind::Attachment, FileMeta {
            file_name: Some( "random.png".to_owned() ),
            creation_date: Some( DateTime::test_time( 1 ) ),
            modification_date: Some( DateTime::test_time( 2 ) ),
            read_date: Some( DateTime::test_time( 3 ) ),
            size: Some( 4096 )
        })
    } => ascii => [
        Text concat!( "attachment",
            ";filename=random.png",
            ";creation-date=\"Tue,  6 Aug 2013 04:11:01 +0000\"",
            ";modification-date=\"Tue,  6 Aug 2013 04:11:02 +0000\"",
            ";read-date=\"Tue,  6 Aug 2013 04:11:03 +0000\"",
            ";size=4096" ),
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