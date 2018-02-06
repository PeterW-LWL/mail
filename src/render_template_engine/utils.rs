use std::mem::replace;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

use super::error::SpecError;

pub(crate) fn string_path_set(field: &mut String, new_path: &Path) -> Result<PathBuf, SpecError> {
    let path = new_string_path(new_path)?;
    let old = replace(field, path);
    Ok(PathBuf::from(old))
}

pub(crate) fn new_string_path<S>(path: S) -> Result<String, SpecError>
    where S: AsRef<OsStr>
{
    let path = path.as_ref();
    if let Some(path) = path.to_str() {
        Ok(path.to_owned())
    } else {
        Err(SpecError::NonStringPath(path.to_owned().into()))
    }
}

pub(crate) fn check_string_path(path: &Path) -> Result<(), SpecError> {
    if path.to_str().is_none() {
        Err(SpecError::NonStringPath(path.to_owned()))
    } else {
        Ok(())
    }
}


pub(crate) fn sniff_media_type(path: &Path) -> Result<MediaType, SpecError> {
    // 1. determine media type by file ending
    // 2. determine media type by file
    // 3. compare (FEAT: validate instead of 2+3, as it only needs to run some tests)
    unimplemented!()

}
