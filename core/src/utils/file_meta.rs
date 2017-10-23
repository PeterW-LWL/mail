

use super::DateTime;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct FileMeta {
    pub file_name: Option<String>,
    pub creation_date: Option<DateTime>,
    pub modification_date: Option<DateTime>,
    pub read_date: Option<DateTime>,
    pub size: Option<usize>
}