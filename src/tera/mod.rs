use std::path::Path;

use tera::{self, Tera, TesterFn, FilterFn, GlobalFn};
use ::template_engine_prelude::*;
use render_template_engine::RenderEngine;

pub struct TeraRenderEngine {
    tera: Tera
}

type Error = <TeraRenderEngine as RenderEngine>::Error;

impl TeraRenderEngine {

    /// create a new TeraRenderEngine given a base_templates_dir
    ///
    /// The `base_templates_glob` contains a number of tera templates which can be used to
    /// inherit (or include) from e.g. a `base_mail.html` which is then used in all
    /// `mail.html` templates through `{% extends "base_mail.html" %}`.
    ///
    /// The `base_templates_glob` _is separate from template dirs used by
    /// the `RenderTemplateEngine`_. It contains only tera templates to be reused at
    /// other places.
    ///
    pub fn new(base_templats_glob: &str) -> Result<Self, Error> {
        let tera = Tera::new(str_path)?;

        Ok(TeraRenderEngine { tera })
    }

    /// Reloads all base templates, but no `RenderTemplateEngine` specific templates.
    /// After a reload `RenderTemplateEngine` specific templates will be loaded when
    /// they are used the next time.
    ///
    pub fn reload_base_only(&mut self) -> Result<Self, Error> {
        //full_reload doe NOT a full reload what it does is
        // 1. discard all templates which are from a Tera::extend call
        //    (yes you can't reload them at all)
        // 2. load all templates from a glob
        //
        // No template path is used at all, even through all templates do have path's assigned
        // them if they where added through a path, well this actually happens to be exactly what
        // we want even through it's not what it says it is.
        self.tera.full_reload()
    }

    /// expose `Tera::register_filter`
    pub fn register_filter(&mut self, name: &str, filter: FilterFn) {
        self.tera.register_filter(name, filter);
    }

    /// exposes `Tera::register_tester`
    pub fn register_tester(&mut self, name: &str, tester: TesterFn) {
        self.tera.register_tester(name, tester);
    }

    /// exposes `Tera::register_global_function`
    pub fn register_global_function(&mut self, name: &str, function: GlobalFn) {
        self.tera.register_global_function(name, function)
    }

    /// exposes `Tera::autoescape_on`
    pub fn set_autoescape_file_suffixes(&mut self, suffixes: Vec<&'static str>) {
        self.tera.autoescape_on(suffixes)
    }
}


impl RenderEngine for TeraRenderEngine {
    // nothing gurantees that the templates use \r\n, so by default fix newlines
    // but it can be disabled
    const PRODUCES_VALID_NEWLINES: bool = false;
    type Error = tera::Error;

    fn render<D: Serialize>(&self, id: &str, data: D) -> StdResult<String, Self::Error> {
        if !self.tera.templates.contains_key(id) {
            // the id used is actually a path to them template defined in TemplateSpec, so
            // we can just add the template if it does not exists
            self.tera.add_template_file(None, id)?;
        }
        self.tera.render(id, data)
    }
}
