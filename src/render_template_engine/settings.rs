use std::rc::Rc;
use std::collections::HashMap;
use std::path::Path;

use media_type::CHARSET;

use mail::MediaType;

use super::error::SpecError;
use super::utils;


//Type::find_media_type(Path)
//   combines type e.g. for html => application/html
//   with looking at the file ending, and more?
//   needs:
//      the name -> media_type mapping
//      some form of charset setting/detection
//Type::alias_names()
//   list of alias names e.g. mail.html, mail.htm,
//   needs:
//      list of aliases file endings


//IMPLEMENTATION NOTE: for now this is a simple configurabe think,
// BUT in the future it can either be
// 1. extended to support more stuff
// 2. made into a trait with multiple impl. where the current impl. is just the default

pub struct Settings {
    type_lookup: HashMap<String, Rc<Type>>,
}

impl Settings {

    pub fn get_type(&self, name: &str) -> Option<&Type> {
        self.type_lookup.get(name).map(|rrc| &**rrc)
    }

    #[inline]
    pub fn determine_media_type<P>(&self, path: P) -> Result<MediaType, SpecError>
        where P: AsRef<Path>
    {
        utils::sniff_media_type(path.as_ref())
    }
}

pub struct Type {
    base_type: String,
    base_subtype: String,
    suffixes: Vec<String>,
    charset: String
}

impl Type {

    pub fn to_media_type_for<P>(&self, path: P) -> Result<MediaType, SpecError>
        where P: AsRef<Path>
    {
        self._as_media_type_for(path.as_ref())
    }

    fn _as_media_type_for(&self, _path: &Path) -> Result<MediaType, SpecError> {
        //FEAT: consider charset sniffing or validate sniffing, allow other parameters for more
        // unusual bodies
        // for now this is just creating a media type and set a preset charset,
        // not trying to verify the charset or anything else
        MediaType::new_with_params(&self.base_type, &self.base_subtype, vec![
            (CHARSET, &self.charset)
        ]).map_err(|e| {
            SpecError::BodyMediaTypeCreationFailure(Box::new(e))
        })
    }

    pub fn suffixes(&self) -> &[String] {
        &self.suffixes
    }

    pub fn template_base_name(&self) -> &str {
        "mail"
    }
}