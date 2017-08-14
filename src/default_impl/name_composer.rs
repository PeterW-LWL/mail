
use mail_composition::NameComposer;


#[derive(Debug, Clone, Copy)]
pub struct NoNameComposer;


impl<D> NameComposer<D> for NoNameComposer {
    fn compose_name( &self, _data: &D ) -> Option<String> {
        None
    }
}