use std::sync::Arc;

use template::TemplateEngine;
use context::Context;

use super::CompositionBase;

pub struct SimpleCompositionBase<CTX, TE> {
    template_engine: TE,
    context: CTX,
    //TODO CONTINUE add phantom shit
}
impl<CTX, TE> SimpleCompositionBase<CTX, TE>
    where TE: TemplateEngine<CTX>, CTX: Context
{
    pub fn new(context: CTX, template_engine: TE) -> Self {
        SimpleCompositionBase { template_engine, context }
    }


    pub fn context_mut(&mut self) -> &mut CTX {
        &mut self.context
    }

    pub fn template_engine_mut(&mut self) -> &TE {
        &mut self.template_engine
    }
}


impl<CTX, TE> CompositionBase for SimpleCompositionBase<CTX, TE>
    where TE: TemplateEngine<CTX>, CTX: Context
{
    type Context = CTX;
    type TemplateEngine = TE;

    fn template_engine(&self) -> &Self::TemplateEngine {
        &self.template_engine
    }
    fn context(&self) -> &Self::Context {
        &self.context
    }
}

pub struct SharedCompositionBase<CTX, TE> {
    template_engine: Arc<TE>,
    context: CTX
}

impl<TE, CTX> SharedCompositionBase<CTX, TE>
    where TE: TemplateEngine<CTX>, CTX: Context
{
    pub fn new(context: CTX, template_engine: TE) -> Self {
        SharedCompositionBase {
            template_engine: Arc::new(template_engine),
            context
        }
    }
}

impl<TE, CTX> CompositionBase for SharedCompositionBase<CTX, TE>
    where TE: TemplateEngine<CTX>, CTX: Context
{
    type TemplateEngine = TE;
    type Context = CTX;

    fn template_engine(&self) -> &Self::TemplateEngine { &self.template_engine }
    fn context(&self) -> &Self::Context { &self.context }
}


impl<'a, TE: 'a, CTX: 'a> CompositionBase for (&'a CTX, &'a TE)
    where TE: TemplateEngine<CTX>, CTX: Context
{
    type Context = CTX;
    type TemplateEngine = TE;

    fn template_engine(&self) -> &Self::TemplateEngine { self.1 }
    fn context(&self) -> &Self::Context { self.0 }
}


