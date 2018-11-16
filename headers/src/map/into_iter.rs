use std::vec;

use ::HeaderName;
use ::header::HeaderObj;

use super::HeaderMap;

pub type IntoIter = vec::IntoIter<(HeaderName, Box<HeaderObj>)>;

impl IntoIterator for HeaderMap {

    type Item = (HeaderName, Box<HeaderObj>);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_map.into_iter()
    }
}