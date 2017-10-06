use std::vec;

use codec::{ MailEncoder, MailEncodable };
use headers::HeaderName;

use super::HeaderMap;

impl<E> IntoIterator for HeaderMap<E>
    where E: MailEncoder
{
    type Item = (HeaderName, Box<MailEncodable<E>>);
    type IntoIter = vec::IntoIter<(HeaderName, Box<MailEncodable<E>>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_map.into_iter()
    }
}



#[cfg(test)]
mod test {
    use codec::{ MailEncodable, MailEncoderImpl};
    use headers::{ To, Subject, From};
    use components::Unstructured;
    use super::HeaderMap;

    #[test]
    fn into_iter() {
        const TEST_TEXT: &str = "this is a subject";
        let mut headers = HeaderMap::<MailEncoderImpl>::new();
        headers.insert(To, [ "affen@haus" ]).unwrap();
        headers.insert(Subject, TEST_TEXT).unwrap();
        headers.insert(From, [ "nix@da", "ja@wohl" ]).unwrap();

        let headers = headers.into_iter()
            .map(|(name, val)| {
                (name.as_str(), val)
            })
            .collect::<Vec<_>>();
        assert_eq!(3, headers.len());
        //check order
        assert_eq!("To", headers[0].0 );
        assert_eq!("Subject", headers[1].0);
        assert_eq!("From", headers[2].0 );

        //check if we can use the data
        let obj: &MailEncodable<_> = &*headers[1].1;
        let text = obj.downcast_ref::<Unstructured>().unwrap();
        assert_eq!(
            TEST_TEXT,
            text.as_str()
        );

    }
}