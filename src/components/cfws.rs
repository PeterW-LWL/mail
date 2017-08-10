use error::*;
use codec::{ MailEncoder, MailEncodable };

//FEATURE_TODO(fws_controll): allow controlling the amount of WS and if a CRLF should be used in FWS
//  this is also usefull for parsing and keeping information about FWS structure
//FEATURE_TODO(cfws_with_comments): allow encoding commnets in CFWS
//  this allows encoding comments in CFWS, combine with (part of?) fws_controll
//  required (partially) for parsing comments (through skipping them works without this)

//
//pub enum WS {
//    TAB,
//    SPACE
//}
//
//pub struct FWS(pub Option<WithCRLF>, pub Vec1<WS> );
//
//pub struct WithCRLF {
//    pub trailing: Vec<WS>
//}

#[derive(Debug, Hash, Clone, Eq, PartialEq, Serialize)]
pub struct FWS;

//NOTE(IMPORTANT): when implementing this I have to assure that encoding CFWS followed by FWS works
// mainly after using a CR-LF-WSP seq you CAN NOT have another FWS which uses unsolds to a CR-LF-WSP
// currently we only remember the last FWS and do only make it in a CR-LF-SPACE sequence when we
// need to, so no problem here for now.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub enum CFWS {
    //WithComment( Vec1<(Option<FWS>, Comment)>, Option<FWS> ),
    SingleFws( FWS )
}


impl MailEncodable for CFWS {

    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        match *self {
            CFWS::SingleFws(ref _fws ) => {
                encoder.write_fws();
            }
        }
        Ok( () )
    }

}

#[cfg(test)]
mod test {
    use super::*;
    use codec::{ test_utils as t };

    ec_test!{ simple_encode,
        {
            Some( CFWS::SingleFws( FWS ) )
        } => utf8 => [
            t::FWS
        ]
    }

}