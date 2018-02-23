use std::sync::Arc;

use rand::{ self, Rng };

use core::error::Result;
use core::utils::HeaderTryFrom;
use headers::components::MessageID;

use context::ContentIdGenComponent;

#[derive( Debug, Clone, Hash, Eq, PartialEq )]
pub struct RandomContentId {
    postfix: Arc<str>
}

impl RandomContentId {

    pub fn new<I>( postfix: I ) -> Self
        where I: Into<String>
    {
        let string = postfix.into();
        let boxed = string.into_boxed_str();
        let arced = Arc::from(boxed);
        RandomContentId { postfix: arced }
    }

}


impl ContentIdGenComponent for RandomContentId {


    fn new_content_id( &self ) -> Result<MessageID> {
        let mut rng = rand::thread_rng();
        let mut msg_id = rng.gen_ascii_chars().take( 10 ).collect::<String>();
        msg_id.push( '@' );
        msg_id += &*self.postfix;
        MessageID::try_from(msg_id)
    }
}


