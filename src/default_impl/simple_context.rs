

use futures_cpupool::{CpuPool, Builder};

use context::CompositeContext;
use default_impl::RandomContentId;
use ::error::Error;
use mail::context::CompositeBuilderContext;
use mail::default_impl::FsResourceLoader;

pub fn new<I: Into<String>>(content_id_postfix: I) -> Result<SimpleContext, Error> {
    Ok(CompositeContext::new(
        RandomContentId::new(content_id_postfix),
        CompositeBuilderContext::new(
            FsResourceLoader::with_cwd_root()?,
            Builder::new().create()
        )
    ))
}

pub type SimpleContext = CompositeContext<RandomContentId,
    CompositeBuilderContext<FsResourceLoader, CpuPool>>;


