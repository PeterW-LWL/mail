//FIXE[rustc/macro reexport] re-export the macro once possible in stable
// currently this won't work well so this is just a copy of the macro in
// mail-headers ;=(
/// Create a header map from a list of header's with ther fields
///
/// # Example
///
/// ```
/// # #[macro_use]
/// # extern crate mail_headers;
/// # use mail_headers::headers::*;
/// # use mail_headers::error::ComponentCreationError;
/// # fn main() { (|| -> Result<(), ComponentCreationError> {
/// let map = headers! {
///     _From: ["bobo@nana.test"],
///     Subject: "hy there"
/// }?;
/// # Ok(()) })(); }
/// ```
#[macro_export]
macro_rules! headers {
    ($($header:ty : $val:expr),*) => ({
        //FIXME[rust/catch block] use catch block once available
        (|| -> Result<$crate::HeaderMap, ::mail::error::ComponentCreationError>
        {
            let mut map = ::mail::HeaderMap::new();
            $(
                map.insert(<$header as ::mail::HeaderKind>::body($val)?);
            )*
            Ok(map)
        })()
    });
}