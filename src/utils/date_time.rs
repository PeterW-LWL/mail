
use chrono;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DateTime( chrono::DateTime<chrono::Utc> );

impl DateTime {
    pub fn new<TZ: chrono::TimeZone>( date_time: chrono::DateTime<TZ>) -> DateTime {
        DateTime( date_time.with_timezone( &chrono::Utc ) )
    }

    #[cfg(test)]
    pub fn test_time( modif: u32 ) -> Self {
        use chrono::prelude::*;
        Self::new( FixedOffset::east( 3 * 3600 ).ymd( 2013, 8, 6 ).and_hms( 7, 11, modif ) )
    }
}

deref0!{-mut DateTime => chrono::DateTime<chrono::Utc> }
