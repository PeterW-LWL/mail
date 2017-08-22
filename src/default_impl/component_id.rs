
use rand::{ self, Rng };

use error::*;
use components::MessageID;
use data::FromInput;

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
        MessageID::from_input( msg_id )
    }
}


