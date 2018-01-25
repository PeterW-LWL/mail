
use rand::{ self, Rng };

use core::error::Result;
use core::utils::HeaderTryFrom;
use mheaders::components::MessageID;

use composition::ContentIdGen;

#[derive( Debug, Clone, Hash, Eq, PartialEq )]
pub struct RandomContentId {
    postfix: String
}

impl RandomContentId {

    pub fn new( postfix: String ) -> Self {
        RandomContentId { postfix }
    }

}


impl ContentIdGen for RandomContentId {


    fn new_content_id( &self ) -> Result<MessageID> {
        let mut rng = rand::thread_rng();
        let mut msg_id = rng.gen_ascii_chars().take( 10 ).collect::<String>();
        msg_id.push( '@' );
        msg_id += &*self.postfix;
        MessageID::try_from(msg_id)
    }
}


