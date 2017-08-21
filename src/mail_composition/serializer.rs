use std::cell::{ RefCell, Ref };
use std::result::{ Result as StdResult };

use serde::{ self, Serialize, Serializer };

use error::*;
use components::MessageID;
use mail::resource::Resource;
use mail_composition::ContentIdGen;


#[derive(Debug)]
pub struct Embedding {
    resource: Resource,
    content_id: RefCell<Option<MessageID>>
}

#[derive(Debug)]
pub struct Attachment {
    resource: Resource
}

impl Embedding {
    pub fn new( resource: Resource ) -> Self {
        Embedding { resource, content_id: RefCell::new( None ) }
    }

    pub fn with_content_id( resource: Resource, content_id: MessageID ) -> Self {
        Embedding {
            resource: resource,
            content_id: RefCell::new( Some( content_id ) )
        }
    }

    pub fn resource( &self ) -> &Resource {
        &self.resource
    }

    pub fn resource_mut( &mut self ) -> &mut Resource {
        &mut self.resource
    }

    pub fn content_id( &self ) -> Option<Ref<MessageID>> {
        let borrow = self.content_id.borrow();
        if borrow.is_some() {
            Some( Ref::map( borrow, |opt_content_id| {
                opt_content_id.as_ref().unwrap()
            } ) )
        } else {
            None
        }
    }

    pub fn set_content_id( &mut self, cid: MessageID ) {
        self.content_id = RefCell::new( Some( cid ) )
    }

    pub fn has_content_id( &self ) -> bool {
        self.content_id.borrow().is_some()
    }

    fn assure_content_id<CIDGen: ContentIdGen + ?Sized>(
        &self,
        cid_gen: &CIDGen
    ) -> Result<MessageID> {

        let mut cid = self.content_id.borrow_mut();
        Ok( if cid.is_some() {
            //UNWRAP_SAFE: would use if let Some, if we had non lexical livetimes
            cid.as_ref().unwrap().clone()
        } else {
            let new_cid = cid_gen.new_content_id()?;
            *cid = Some( new_cid.clone() );
            new_cid
        } )
    }
}


impl Attachment {
    pub fn new(resource: Resource) -> Self {
        Attachment { resource }
    }

    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
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
            match self.assure_content_id( &*dump.cid_gen ) {
                Ok( cid ) => {
                    let ser_res = serializer.serialize_str( cid.as_str() );
                    if ser_res.is_ok() {
                        // Resource is (now) meant to be shared, and cloning is sheap (Arc inc)
                        let resource = self.resource().clone();
                        dump.embeddings.push( (cid, resource) );
                    }
                    ser_res
                },
                Err( err ) => {
                    Err( serde::ser::Error::custom( err ) )
                }
            }
        } )
    }
}


impl Serialize for Attachment {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error> where S: Serializer {
        if !EXTRACTION_DUMP.is_set() {
            return Err( serde::ser::Error::custom(
                "can only serialize an Attachment in when wrapped with serialize_and_extract" ) );
        }
        EXTRACTION_DUMP.with( |dump: &RefCell<ExtractionDump>| {
            let mut dump = dump.borrow_mut();
            let ser_res = serializer.serialize_none();
            if ser_res.is_ok() {
                let resource = self.resource.clone();
                dump.attachments.push( resource );
            }
            ser_res
        })
    }
}

impl Into<Resource> for Embedding {
    fn into( self ) -> Resource {
        self.resource
    }
}

impl Into<Resource> for Attachment {
    fn into( self ) -> Resource {
        self.resource
    }
}


struct ExtractionDump {
    embeddings: Vec<(MessageID, Resource)>,
    attachments: Vec<Resource>,
    //BLOCKED(unsized_thread_locals): use ContentIdGen instead in another thread_local when possible
    cid_gen: Box<ContentIdGen>
}

scoped_thread_local!(static EXTRACTION_DUMP: RefCell<ExtractionDump> );


#[derive(Serialize)]
pub struct SerializeOnly<T: Serialize> {
    data: T
}

///
/// use this to get access to Embedding/Attachment Resources while serializing
/// structs containing the Embedding/Attachment types. This also includes the
/// gneration of a ContentId for embeddings which do not jet have one.
///
/// This might be a strange approach, but this allows you to "just" embed Embedding/Attachment
/// types in data struct and still handle them correctly when passing them to a template
/// engine without having to explicitly implement a trait allowing interation of all
/// (even transistive) contained Embeddings/Attachments
///
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
