use std::mem::replace;
use std::result::{ Result as StdResult };

use serde;
use serde::ser::Error;

use error::*;
use components::{ Mailbox, MessageID };
use mail::resource::Resource;

use super::resource::{
    Embeddings, Attachments,
    EmbeddingInMail,
};
use super::context::{
    Context
};



pub trait DataInterface: serde::Serialize {

    /// calls the function visit_emb with all embeddings contained
    /// in the data and the function visit_att with all attachments
    /// contained in the data, if any call of visit_emb/visit_att
    /// fails with an error the error is returned
    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>;

    fn see_from_mailbox(&mut self, _mbox: &Mailbox ) {}
    fn see_to_mailbox(&mut self, _mbox: &Mailbox ) {}
}



#[derive(Debug, Serialize)]
pub struct EmbeddingInData( InnerEmbedding );

#[derive(Debug)]
enum InnerEmbedding {
    AsValue( Resource ),
    AsContentId( MessageID )
}

impl EmbeddingInData {
    pub fn new( resource: Resource ) -> Self {
        EmbeddingInData( InnerEmbedding::AsValue( resource ) )
    }

    //TODO access methods for the AsValue variant

    fn swap_with_content_id( &mut self, cid: MessageID ) -> Option<Resource> {
        use self::InnerEmbedding::*;
        match replace( &mut self.0, AsContentId( cid ) ) {
            //TODO warn this is definitily a bug
            AsContentId( _cid ) => None,
            AsValue( value ) => Some( value )
        }
    }
}

impl serde::Serialize for InnerEmbedding {
    fn serialize<S>( &self, serializer: S ) -> StdResult<S::Ok, S::Error>
        where S: serde::Serializer
    {
        use serde::ser::Error;
        use self::InnerEmbedding::*;
        match *self {
            AsValue( .. ) => Err( S::Error::custom( concat!(
                "embeddings can be serialized as content id, not as value, ",
                "preprocess_data should have ben called before" ) ) ),
            //FIXME potentialy use cid encode as string!
            AsContentId( ref cid ) => cid.serialize( serializer )
        }
    }
}


#[derive(Debug, Serialize)]
pub struct AttachmentInData( InnerAttachment );

#[derive(Debug)]
enum InnerAttachment {
    AsValue( Resource ),
    /// the resource was moved out of data, to be added to the
    /// mail attachments
    Moved
}

impl AttachmentInData {
    pub fn new( resource: Resource ) -> Self {
        AttachmentInData( InnerAttachment::AsValue( resource ) )
    }

    //TODO access methods for the AsValue variant

    fn move_out( &mut self ) -> Option<Resource> {
        use self::InnerAttachment::*;
        match replace( &mut self.0, InnerAttachment::Moved ) {
            AsValue( value ) => Some( value ),
            //TODO warn as this is likely a bug
            Moved => None
        }
    }
}

impl serde::Serialize for InnerAttachment {
    fn serialize<S>( &self, serializer: S ) -> StdResult<S::Ok, S::Error>
        where S: serde::Serializer
    {
        use self::InnerAttachment::*;
        match *self {
            AsValue( .. ) => Err( S::Error::custom( concat!(
                "only moved attachments can be serialized, ",
                "preprocess_data should have ben called before" ) ) ),
            Moved => serializer.serialize_none()
        }
    }
}

pub fn preprocess_data<C: Context, D: DataInterface>( ctx: &C, data: &mut D )
    -> Result<(Embeddings, Attachments)>
{
    let mut embeddings = Vec::new();
    let mut attachments = Vec::new();
    data.find_externals(
        &mut |embedding| {
            //FEATURE_TODO(context_sensitive_content_id): pass ing &data
            let new_cid = ctx.new_content_id()?;
            if let Some( embedding ) = embedding.swap_with_content_id( new_cid.clone() ) {
                embeddings.push( EmbeddingInMail {
                    content_id: new_cid,
                    resource: embedding
                } )
            }
            Ok( () )
        },
        &mut |attachment| {
            if let Some( attachment ) = attachment.move_out() {
                attachments.push( attachment )
            }
            Ok( () )
        }
    )?;

    Ok( (embeddings, attachments) )
}


#[cfg(test)]
mod test {
    use data::FromInput;
    use mail::mime::SinglepartMime;
    use std::path::PathBuf;
    use mime::TEXT_PLAIN;
    use super::*;


    #[test]
    fn aid_move_out() {
        let mut attachment = AttachmentInData::new( Resource::File {
            mime: SinglepartMime::new( TEXT_PLAIN ).unwrap(),
            path: PathBuf::from( "/does/not/exist" ),
            alternate_name: None,
        });

        let resource = attachment.move_out();
        if let Some( Resource::File { .. } ) = resource {
        } else {
            panic!( "move_out should have returned a resource")
        }

        if let AttachmentInData( InnerAttachment::AsValue( .. ) ) = attachment {
            panic!( "the resource should have been moved out of the attachment type" )
        }
    }

    #[test]
    fn eid_swap_with_content_id() {
        let mut embedding = EmbeddingInData::new( Resource::File {
            mime: SinglepartMime::new( TEXT_PLAIN ).unwrap(),
            path: PathBuf::from( "/does/not/exist" ),
            alternate_name: None,
        });

        let resource = embedding.swap_with_content_id(
            MessageID::from_input( "abc@def" ).unwrap()
        );
        if let Some( Resource::File { .. } ) = resource {
        } else {
            panic!( "swap should have returned a resource")
        }

        if let EmbeddingInData( InnerEmbedding::AsValue( .. ) ) = embedding {
            panic!( "the resource should have been moved out of the embedding type" )
        }
    }



}