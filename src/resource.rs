use std::cell::{ RefCell, Ref };

use serde::{ self, Serialize, Serializer };

use headers::components::ContentId;
use mail::Resource;
use mail::context::MailIdGenComponent;


//TODO consider to rename it to "in-data-embedding"
#[derive(Debug)]
pub struct Embedding {
    resource: Resource,
    content_id: RefCell<Option<ContentId>>
}

#[derive(Debug)]
pub struct EmbeddingWithCId {
    resource: Resource,
    content_id: ContentId
}

#[derive(Debug)]
pub struct Attachment {
    resource: Resource
}

impl Embedding {
    pub fn new(resource: Resource) -> Self {
        Embedding { resource, content_id: RefCell::new(None) }
    }

    pub fn with_content_id(resource: Resource, content_id: ContentId) -> Self {
        Embedding {
            resource: resource,
            content_id: RefCell::new(Some(content_id))
        }
    }

    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
    }

    pub fn content_id(&self) -> Option<Ref<ContentId>> {
        let borrow = self.content_id.borrow();
        if borrow.is_some() {
            Some(Ref::map(borrow, |opt_content_id| {
                opt_content_id.as_ref().unwrap()
            }))
        } else {
            None
        }
    }

    pub fn set_content_id(&mut self, cid: ContentId) {
        self.content_id = RefCell::new(Some(cid))
    }

    pub fn has_content_id( &self ) -> bool {
        self.content_id.borrow().is_some()
    }

    pub fn with_cid_assured(self, ctx: &impl MailIdGenComponent) -> EmbeddingWithCId {
        let Embedding { resource, content_id } = self;
        let content_id =
            if let Some( cid ) = content_id.into_inner() {
                cid
            } else {
                ctx.generate_content_id()
            };
        EmbeddingWithCId { resource, content_id }
    }

    fn assure_content_id(&self, ctx: &(impl MailIdGenComponent + ?Sized)) -> ContentId {
        let mut cid = self.content_id.borrow_mut();
        if cid.is_some() {
            //UNWRAP_SAFE: would use if let Some, if we had non lexical livetimes
            cid.as_ref().unwrap().clone()
        } else {
            let new_cid = ctx.generate_content_id();
            *cid = Some( new_cid.clone() );
            new_cid
        }
    }
}

impl EmbeddingWithCId {
    pub fn new(resource: Resource, content_id: ContentId) -> Self {
        EmbeddingWithCId { resource, content_id }
    }

    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
    }

    pub fn content_id(&self) -> &ContentId {
        &self.content_id
    }

    pub fn content_id_mut(&mut self) -> &mut ContentId {
        &mut self.content_id
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
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        side_channel::with_dump_do(|dump| {
            let content_id = self.assure_content_id(dump.id_gen);
            let ser_res = serializer.serialize_str(content_id.as_str());
            if ser_res.is_ok() {
                // Resource is (now) meant to be shared, and cloning is cheap (Arc inc)
                let resource = self.resource().clone();
                dump.embeddings.push(EmbeddingWithCId { resource, content_id })
            }
            ser_res
        })
    }
}

impl Serialize for EmbeddingWithCId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut serializer = Some(serializer);
        let res = side_channel::with_dump_do_if_available(|dump| {
            let serializer = serializer.take().unwrap();
            let ser_res = serializer.serialize_str(self.content_id.as_str());
            if ser_res.is_ok() {
                // Resource is (now) meant to be shared, and cloning is cheap (Arc inc)
                let resource = self.resource().clone();
                dump.embeddings.push(EmbeddingWithCId {
                    resource, content_id: self.content_id.clone()
                });
            }
            ser_res
        });

        if let Some(res) = res {
            res
        } else {
            let serializer = serializer.take().unwrap();
            serializer.serialize_str(self.content_id.as_str())
        }
    }
}


impl Serialize for Attachment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        side_channel::with_dump_do(|dump| {
            let ser_res = serializer.serialize_none();
            if ser_res.is_ok() {
                let resource = self.resource.clone();
                dump.attachments.push(Attachment::new(resource));
            }
            ser_res
        })
    }
}

impl Into<Resource> for Embedding {
    fn into(self) -> Resource {
        self.resource
    }
}

impl From<Resource> for Embedding {
    fn from(r: Resource) -> Self {
        Embedding::new(r)
    }
}

impl Into<Resource> for Attachment {
    fn into(self) -> Resource {
        self.resource
    }
}

impl From<Resource> for Attachment {
    fn from(r: Resource) -> Self {
        Attachment::new(r)
    }
}

impl Into<(ContentId, Resource)> for EmbeddingWithCId {
    fn into(self) -> (ContentId, Resource) {
        (self.content_id, self.resource)
    }
}


pub use self::side_channel::with_resource_sidechanel;
mod side_channel {
    use std::mem;
    use super::*;
    use mail::context::MailIdGenComponent;

    pub(crate) struct ExtractionDump<'a> {
        pub(crate) embeddings: Vec<EmbeddingWithCId>,
        pub(crate) attachments: Vec<Attachment>,
        pub(crate) id_gen: &'a MailIdGenComponent
    }

    /// this is 'static due to rust limitations, but in praxis it is a `'a` which
    /// lives long enough for any scope it can appear at runtime _but this `'a` is
    /// likely much smaller then `'static`_. The functions accesing it take additional
    /// measurements to make sure that it won't be misused and the user never sees this
    /// `'static`
    scoped_thread_local!(static EXTRACTION_DUMP: RefCell<ExtractionDump<'static>>);

    ///
    /// use this to get access to Embedding/Attachment Resources while serializing
    /// structs containing the Embedding/Attachment types. This also includes the
    /// generation of a ContentId for embeddings which do not jet have one.
    ///
    /// This might be a strange approach, but this allows you to "just" embed Embedding/Attachment
    /// types in data struct and still handle them correctly when passing them to a template
    /// engine without having to explicitly implement a trait allowing iteration of all
    /// (even transitive) contained Vec<Embeddings>/Vec<Attachment>
    ///
    /// # Returns
    ///
    /// returns a 3-tuple of the result of the function passed in (`func`) a vector
    /// of Embeddings (as content id, Resource pairs) and a vector of attachments
    /// ( as Resource's )
    ///
    /// # Note
    /// this function is meant for internal use of mail composition "algorithm"
    /// the reason why it is public is so that other/custom composition code can use it, too.
    ///
    pub fn with_resource_sidechanel<'a, R, E>(
        id_gen: &'a impl MailIdGenComponent,
        func: impl FnOnce() -> Result<R, E>
    ) -> Result<(R, Vec<EmbeddingWithCId>, Vec<Attachment> ), E> {
        let dump: ExtractionDump<'a> = ExtractionDump {
            id_gen,
            embeddings: Default::default(),
            attachments: Default::default()
        };
        //SAFE: it is just keept during this function and access to it is only through a shorter lifetime
        let cast_dump: ExtractionDump<'static> = unsafe { mem::transmute(dump) };
        let dump_cell = RefCell::new(cast_dump);
        match EXTRACTION_DUMP.set(&dump_cell, func) {
            Ok(result) => {
                let ExtractionDump { id_gen:_, embeddings, attachments } = dump_cell.into_inner();
                Ok((result, embeddings, attachments))
            },
            Err(err) => Err(err)
        }

    }

    pub(crate) fn with_dump_do<FN, V, ES>(func: FN) -> Result<V, ES>
        where FN: for<'a> FnOnce(&'a mut ExtractionDump<'a>) -> Result<V, ES>,
              ES: serde::ser::Error
    {
        if !EXTRACTION_DUMP.is_set() {
            return Err(serde::ser::Error::custom(
                "can only serialize an Attachment in data when wrapped with with_resource_sidechanel" ) );
        }

        _with_dump_do(func)
    }

    pub(crate) fn with_dump_do_if_available<FN, R>(func: FN) -> Option<R>
        where FN: for<'a> FnOnce(&'a mut ExtractionDump<'a>) -> R
    {
        if EXTRACTION_DUMP.is_set() {
            Some(_with_dump_do(func))
        } else {
            None
        }
    }

    fn _with_dump_do<FN, R>(func: FN) -> R
        where FN: for<'a> FnOnce(&'a mut ExtractionDump<'a>) -> R
    {
        EXTRACTION_DUMP.with(|dump: &RefCell<ExtractionDump<'static>>| {
            let dump: &mut ExtractionDump<'static> = &mut *dump.borrow_mut();
            func(lt_fix(dump))
        })
    }

    fn lt_fix<'a>(val: &'a mut ExtractionDump<'static>) -> &'a mut ExtractionDump<'a>
        where 'static: 'a
    {
        unsafe { mem::transmute(val) }
    }
}




