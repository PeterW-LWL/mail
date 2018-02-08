use std::mem::replace;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::process::Command;
use std::io;

use conduit_mime_types::Types as TypesBySuffix;

use mail::MediaType;

lazy_static! {
    static ref TYPES_BY_SUFFIX: TypesBySuffix = {
        TypesBySuffix::new()
            .expect("embedded json data corrupted in conduit_mime_types")
    };
}

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
    //this does not work for
    // 1. multi part extensions like .tar.gz
    // 2. types not supported by conduit-media-types (which, btw. include .tar.gz /.tgz)
    let extension = path.extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| SpecError::NoValidFileStem(path.to_owned()))?;

    // 1. determine media type by file ending
    let by_extension_str_media_type = TYPES_BY_SUFFIX
        .get_mime_type(extension)
        .ok_or_else(|| SpecError::NoMediaTypeFor(extension.to_owned()))?;

    // 2. determine media type by file
    let media_type = sniff_with_file_cmd(path)?;

    // 3. compare (FEAT: validate instead of 2+3, as it only needs to run some tests)
    if media_type.full_type() != by_extension_str_media_type {
        //FEAT if they are close by in the subtyping base media tree it's still fine
        // e.g. if tp by extension is text/plain and tp by file is text/x-tex we can go with
        // text/plain (the file ending). But we might also do so the other way around _iff_
        // we could get the actual probabilities for both the extension based and the file ending based
        // one
        return Err(SpecError::FileStemAndContentDifferInMediaType {
            path: path.to_owned(),
            by_extension: by_extension_str_media_type.to_owned(),
            by_content: media_type.full_type().to_string()
        });
    }

    Ok(media_type)

}


pub(crate) fn sniff_with_file_cmd(path: &Path) -> Result<MediaType, SpecError> {
    let out = Command::new("file")
        .args(&["-b", "--mime"])
        .arg(path)
        .output()?;

    if !out.status.success() {
        //note: this should normally not happen, expect if there is no file a <path>
        let err_msg = format!(
            concat!(
                "file -b -mime <path> failed with exit code {}\n",
                "path: {path}\n",
                "stderr: {stderr}\n",
                "stdout: {stdout}"
            ),
            path=path.display(),
            stderr=String::from_utf8_lossy(&out.stderr),
            stdout=String::from_utf8_lossy(&out.stdout)
        );
        return Err(io::Error::new(io::ErrorKind::Other, err_msg).into())
    }

    String::from_utf8(out.stdout)
        .map_err(|err| SpecError::NonUtf8MediaType(err))
        .and_then(|string| {
            //FEAT: make parse accept owned data
            MediaType::parse(string.trim())
                .map_err(|err| SpecError::NotAMediaType(err))
        })
}



#[cfg(test)]
mod test {
    use std::path::Path;
    use super::super::error::SpecError;
    use super::sniff_media_type;


    #[test]
    fn sniff_pdf() {
        let mt = sniff_media_type(Path::new("./test_resources/simple.pdf")).unwrap();
        assert_eq!(mt.as_str_repr(), "application/pdf; charset=binary")
    }

    #[test]
    fn sniff_image() {
        let mt = sniff_media_type(Path::new("./test_resources/png_image.png")).unwrap();
        assert_eq!(mt.as_str_repr(), "image/png; charset=binary")
    }

    #[test]
    fn sniff_ascii() {
        let mt = sniff_media_type(Path::new("./test_resources/ascii_text.txt")).unwrap();
        assert_eq!(mt.as_str_repr(), "text/plain; charset=us-ascii")
    }

    #[test]
    fn sniff_utf8() {
        let mt = sniff_media_type(Path::new("./test_resources/utf8_text.txt")).unwrap();
        assert_eq!(mt.as_str_repr(), "text/plain; charset=utf-8")
    }

    #[test]
    fn sniff_conflicting_image() {
        let _path = Path::new("./test_resources/jpg_image.png");
        let err = sniff_media_type(_path).unwrap_err();
        if let SpecError::FileStemAndContentDifferInMediaType { path, by_extension, by_content }
            = err
        {
            assert_eq!(path, _path);
            assert_eq!(by_extension, "image/png");
            assert_eq!(by_content, "image/jpeg");
        } else {
            panic!("unexpected error: {}", err);
        }
    }
}