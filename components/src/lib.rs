#[macro_use]
extern crate mail_codec_core as core;
#[cfg_attr(test, macro_use)]
extern crate vec1;
extern crate mime;
extern crate soft_ascii_string;
#[macro_use]
extern crate nom;

#[macro_use]
extern crate quick_error;

#[macro_use]
mod error;

mod utils;

mod email;
pub use self::email::{ Email, Domain, LocalPart };

mod mailbox;
pub use self::mailbox::{Mailbox, NoDisplayName};

mod mailbox_list;
pub use self::mailbox_list::{MailboxList, OptMailboxList };



mod unstructured;
pub use self::unstructured::Unstructured;

mod message_id;
pub use self::message_id::{ MessageID, MessageIDList };

pub type ContentID = MessageID;
pub type ContentIDList = MessageIDList;


mod cfws;
pub use self::cfws::{ CFWS, FWS };

mod date_time;
pub use self::date_time::DateTime;

pub mod mime_tools;
pub use self::mime::Mime;

mod path;
pub use self::path::Path;

mod received_token;
pub use self::received_token::ReceivedToken;


mod transfer_encoding;

pub mod word;
pub use self::word::Word;

mod phrase;
pub use self::phrase::Phrase;

mod phrase_list;
pub use self::phrase_list::PhraseList;

mod disposition;
pub use self::disposition::*;
