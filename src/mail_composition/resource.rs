use components::MessageID;
use mail::resource::Resource;

pub type Embeddings = Vec<EmbeddingInMail>;
pub type Attachments = Vec<AttachmentInMail>;

pub type AttachmentInMail = Resource;

#[derive(Debug)]
pub struct EmbeddingInMail {
    pub content_id: MessageID,
    pub resource: Resource
}
