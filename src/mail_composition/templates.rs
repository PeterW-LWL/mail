use std::result::{ Result as StdResult };
use std::error::{ Error as StdError };

use serde::Serialize;

use types::Vec1;
use mail::Resource;

use super::context::Context;
use super::resource::{ Attachments, Embeddings };

pub trait TemplateEngine {
    type TemplateId;
    type Error: StdError + Send + 'static;

    fn templates<D: Serialize, C: Context>( &self,  ctx: &C, id: Self::TemplateId, data: D )
                                -> StdResult< Vec1<Template>, Self::Error >;
}


pub struct Template {
    pub body: Resource,
    pub embeddings: Embeddings,
    pub attachments: Attachments
}