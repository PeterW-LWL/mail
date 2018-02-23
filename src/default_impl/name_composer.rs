
use core::error::Result;
use compositor::NameComposer;
use headers::components::Email;


#[derive(Debug, Clone, Copy)]
pub struct NoNameComposer;


impl<D> NameComposer<D> for NoNameComposer {
    fn compose_from_name( &self, _email: &Email, _data: &mut D ) -> Result<Option<String>> {
        Ok(None)
    }
    fn compose_to_name( &self, _email: &Email, _data: &mut D ) -> Result<Option<String>> {
        Ok(None)
    }
}