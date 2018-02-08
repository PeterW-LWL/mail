use std::path::{Path, PathBuf};
use std::collections::HashMap;

use vec1::Vec1;

use mail::ResourceSpec;

use super::error::SpecError;
use super::utils::new_string_path;
use super::{TemplateSpec, SubTemplateSpec};
use super::settings::{Settings, Type};

//TODO missing global template level embeddings
//TODO missing caching (of Resources)


pub(crate) fn from_dir(base_path: &Path, settings: &Settings) -> Result<TemplateSpec, SpecError> {
    let mut glob_embeddings = HashMap::new();
    let mut sub_template_dirs = Vec::new();
    for folder in base_path.read_dir()? {
        let entry = folder?;
        if entry.file_type()?.is_dir() {
            let type_name = entry.file_name()
                .into_string().map_err(|_| SpecError::NonStringPath(entry.path()))?;
            let (prio, type_) = settings.get_type_with_priority(&*type_name)
                .ok_or_else(|| SpecError::MissingTypeInfo(type_name.clone()))?;
            sub_template_dirs.push((prio, entry.path(), type_));
        } else {
            let (name, resource_spec) = embedding_from_path(entry.path(), settings)?;
            glob_embeddings.insert(name, resource_spec);
        }
    }

    sub_template_dirs.sort_by_key(|data| data.0);

    let mut sub_specs = Vec::with_capacity(sub_template_dirs.len());
    for (_, dir_path, type_) in sub_template_dirs {
        sub_specs.push(sub_template_from_dir(&*dir_path, type_, settings)?);
    }

    let sub_specs = Vec1::from_vec(sub_specs)
        .map_err(|_| SpecError::NoSubTemplatesFound(base_path.to_owned()))?;
    TemplateSpec::new_with_embeddings_and_base_path(
        sub_specs, glob_embeddings, base_path.to_owned())
}


//NOTE: if this is provided as a pub utility provide a wrapper function instead which
// only accepts dir_path + settings and gets the rest from it
fn sub_template_from_dir(dir: &Path, type_: &Type, settings: &Settings)
    -> Result<SubTemplateSpec, SpecError>
{
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
            let (key, value) = embedding_from_path(path, settings)?;
            match embeddings.entry(key) {
                Occupied(oe) => return Err(SpecError::DuplicateEmbeddingName(oe.key().clone())),
                Vacant(ve) => {ve.insert(value);}
            }
        }
    }
    Ok(embeddings)
}

fn embedding_from_path(path: PathBuf, settings: &Settings)
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

    let media_type = settings.determine_media_type(&path)?;

    let resource_spec = ResourceSpec {
        path, media_type,
        name: Some(file_name),
    };

    Ok((name, resource_spec))
}