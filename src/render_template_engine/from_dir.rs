use std::path::{Path, PathBuf};
use std::collections::HashMap;

use vec1::Vec1;

use mail::ResourceSpec;

use super::error::SpecError;
use super::utils::new_string_path;
use super::{TemplateSpec, SubTemplateSpec};
use super::settings::{Settings, Type};

pub(crate) fn from_dir(settings: &Settings, base_path: &Path) -> Result<TemplateSpec, SpecError> {
    let mut sub_specs = Vec::new();
    for folder in base_path.read_dir()? {
        let entry = folder?;
        if entry.file_type()?.is_dir() {
            let sub_template = sub_template_from_dir(settings, &*entry.path())?;
            sub_specs.push(sub_template);
        }
    }
    let sub_specs = Vec1::from_vec(sub_specs)
        .map_err(|_| SpecError::NoSubTemplatesFound(base_path.to_owned()))?;
    TemplateSpec::new_with_base_path(sub_specs, base_path.to_owned())
}


fn sub_template_from_dir(settings: &Settings, dir: &Path)
    -> Result<SubTemplateSpec, SpecError>
{
    //UNWRAP_SAFE: returns None if dir ends in "..", it's a read_dir entries dir,
    // so it can not end in ".."
    let file_name = dir.file_name().unwrap();
    let type_name = new_string_path(file_name)?;
    let type_ = settings.get_type(&*type_name)
        .ok_or_else(|| SpecError::MissingTypeInfo(type_name.to_owned()))?;

    let template_file = find_template_file(dir, type_)?;
    let media_type = type_.to_media_type_for(&*template_file)?;
    let embeddings = find_embeddings(dir, &*template_file, settings)?;

    SubTemplateSpec::new(template_file, media_type, embeddings, Vec::new())
}

fn find_template_file(dir: &Path, type_: &Type) -> Result<PathBuf, SpecError> {
    let base_name = type_.template_base_name();
    type_.suffixes()
        .iter()
        .map(|suffix| dir.join(base_name.to_owned() + suffix))
        .find(|path| path.exists())
        .ok_or_else(|| SpecError::TemplateFileMissing(dir.to_owned()))
}


fn find_embeddings(target_path: &Path, template_file: &Path, settings: &Settings)
    -> Result<HashMap<String, ResourceSpec>, SpecError>
{
    use std::collections::hash_map::Entry::*;

    let mut embeddings = HashMap::new();
    for entry in target_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path != template_file {
            let (key, value) = embedding_from_dir_entry(path, settings)?;
            match embeddings.entry(key) {
                Occupied(oe) => return Err(SpecError::DuplicateEmbeddingName(oe.key().clone())),
                Vacant(ve) => {ve.insert(value);}
            }
        }
    }
    Ok(embeddings)
}

fn embedding_from_dir_entry(path: PathBuf, settings: &Settings)
    -> Result<(String, ResourceSpec), SpecError>
{
    if !path.is_file() {
        return Err(SpecError::NotAFile(path.to_owned()));
    }

    let file_name = new_string_path(
        path.file_name()
        // UNWRAP_SAFE: file_name returns the file (,dir,symlink) name which
        // has to exist for a dir_entry
        .unwrap())?;

    let name = file_name.split(".")
        .next()
        //UNWRAP_SAFE: Split iterator has alway at last one element
        .unwrap()
        .to_owned();

    let use_media_type = settings.determine_media_type(&path)?;

    let resource_spec = ResourceSpec {
        path,
        use_name: Some(file_name),
        use_mime: Some(use_media_type),
    };

    Ok((name, resource_spec))
}