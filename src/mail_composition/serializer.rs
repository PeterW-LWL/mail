use std::rc::Rc;
use std::cell::{ RefCell, Ref };
use std::ops::{ Deref, DerefMut };
use std::result::{ Result as StdResult };
use std::fmt::Debug;

use serde::{ self, Serialize, Serializer };

use error::*;
use components::MessageID;
use mail::resource::Resource;
use mail_composition::{
    EmbeddingInMail,
    AttachmentInMail,
    ContentIdGen

};



pub struct Embedding {
    resource: RefCell<Option<Resource>>,
    content_id: RefCell<Option<MessageID>>
}

pub struct Attachment( RefCell<Option<Resource>> );

impl Embedding {
    pub fn new( resource: Resource ) -> Self {
        Embedding {
            resource: RefCell::new( Some( resource ) ),
            content_id: RefCell::new( None )
        }
    }

    pub fn with_content_id( resource: Resource, content_id: MessageID ) -> Self {
        Embedding {
            resource: RefCell::new( Some( resource ) ),
            content_id: RefCell::new( Some( content_id ) )
        }
    }

    pub fn resource( &self ) -> Ref<Resource> {
        Ref::map( self.resource.borrow(), |opt_resource| opt_resource.as_ref().unwrap() )
    }

    pub fn resource_mut( &mut self ) -> &mut Resource {
        self.resource.get_mut().as_mut().unwrap()
    }
    pub fn content_id( &self ) -> Ref<MessageID> {
        Ref::map( self.content_id.borrow(), |opt_cid| opt_cid.as_ref().unwrap() )
    }

    pub fn content_id_mut( &mut self ) -> &mut MessageID {
        self.content_id.get_mut().as_mut().unwrap()
    }

    fn move_out<CIDGen: ContentIdGen+?Sized>( &self, cid_gen: &CIDGen ) -> Result<(MessageID, Resource)> {
        use std::mem::replace;
        let resource = self.resource.borrow_mut();
        let ret_resource = match self.resource.borrow_mut().take() {
            Some( resc ) => resc,
            None => bail!( "extracting resource from empty resource" )
        };

        let mut cid = self.content_id.borrow_mut();
        let ret_cid;

        if cid.is_some() {
            //UNWRAP_SAFE: the only reason we conat use `if let Some` is lexical lifetimes wrt. else
            ret_cid = cid.as_ref().unwrap().clone();
        } else {
            ret_cid = cid_gen.new_content_id()?;
            *cid = Some( ret_cid.clone() );
        }
        Ok( ( ret_cid, ret_resource ) )
    }

}

impl Attachment {
    pub fn new( resource: Resource ) -> Self {
        Attachment( RefCell::new( Some( resource ) ) )
    }

    pub fn resource( &self ) -> Ref<Resource> {
        Ref::map( self.0.borrow(), |opt_resource| opt_resource.as_ref().unwrap() )
    }

    pub fn resource_mut( &mut self ) -> &mut Resource {
        self.0.get_mut().as_mut().unwrap()
    }

    fn move_out( &self ) -> Result<Resource> {
        self.0.borrow_mut()
            .take()
            .ok_or_else( || "extracting resource which was already moved out".into() )
    }
}


struct ExtractionDump {
    embeddings: Vec<(MessageID, Resource)>,
    attachments: Vec<Resource>,
    //BLOCKED(unsized_thread_locals): use ContentIdGen instead in another thread_local when possible
    cid_gen: Box<ContentIdGen>
}

scoped_thread_local!(static EXTRACTION_DUMP: RefCell<ExtractionDump> );

pub fn with_resource_sidechanel<FN, R>( cid_gen: Box<ContentIdGen>, func: FN ) -> R
    where FN: FnOnce() -> R
{
    let dump: RefCell<ExtractionDump> = RefCell::new( ExtractionDump {
        cid_gen,
        embeddings: Default::default(),
        attachments: Default::default()
    } );

    EXTRACTION_DUMP.set( &dump, func )
}



#[derive(Serialize)]
pub struct SerializeOnly<T: Serialize> {
    data: T
}


impl Serialize for Attachment {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error> where S: Serializer {
        if !EXTRACTION_DUMP.is_set() {
            return Err( serde::ser::Error::custom(
                "can only serialize an Attachment in when wrapped with serialize_and_extract" ) );
        }
        EXTRACTION_DUMP.with( |dump: &RefCell<ExtractionDump>| {
            let mut dump = dump.borrow_mut();
            match self.move_out() {
                Ok( resource ) => {
                    dump.attachments.push( resource );
                    serializer.serialize_none()
                },
                Err( e ) => {
                    Err( serde::ser::Error::custom( e ) )
                }
            }
        })
    }
}

impl Serialize for Embedding {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error> where S: Serializer {
        if !EXTRACTION_DUMP.is_set() {
            return Err( serde::ser::Error::custom(
                "can only serialize an Attachment in when wrapped with serialize_and_extract" ) );
        }
        EXTRACTION_DUMP.with( |dump: &RefCell<ExtractionDump>| {
            let mut dump = dump.borrow_mut();
            match self.move_out( &*dump.cid_gen ) {
                Ok( (content_id, resource ) ) => {
                    let res = serializer.serialize_str( content_id.as_str() );
                    dump.embeddings.push( (content_id, resource) );
                    res
                },
                Err( e ) => {
                    Err( serde::ser::Error::custom( e ) )
                }
            }
        })
    }
}