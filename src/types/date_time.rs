
use chrono;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DateTime( chrono::DateTime<chrono::Utc> );

impl DateTime {
    pub fn new<TZ: chrono::TimeZone>( date_time: chrono::DateTime<TZ>) -> DateTime {
        DateTime( date_time.with_timezone( &chrono::Utc ) )
    }
}

deref0!{-mut DateTime => chrono::DateTime<chrono::Utc> }
