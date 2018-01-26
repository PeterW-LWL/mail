use serde::Serialize;

#[derive(Serialize)]
pub struct SerializeOnly<T: Serialize> {
    data: T
}

impl<T: Serialize> SerializeOnly<T> {
    pub fn new( data: T ) -> Self {
        SerializeOnly { data }
    }
}