
#[cfg(feature="default_impl_name_composer")]
mod name_composer;
#[cfg(feature="default_impl_name_composer")]
pub use self::name_composer::*;

#[cfg(feature="default_impl_component_id")]
mod component_id;
#[cfg(feature="default_impl_component_id")]
pub use self::component_id::*;


#[cfg(feature="default_impl_simple_context")]
pub mod simple_context;

