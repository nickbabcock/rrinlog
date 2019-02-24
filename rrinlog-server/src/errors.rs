use chrono::prelude::*;
use diesel::result::ConnectionError;
use diesel::result::Error as DsError;

#[derive(Fail, Debug)]
pub enum DataError {
    #[fail(display = "Unable to connecto database {}: {}", _0, _1)]
    DbConn(String, #[cause] ConnectionError),

    #[fail(display = "Unable to execute query: {}: {}", _0, _1)]
    DbQuery(String, #[cause] DsError),

    #[fail(display = "One target expected: {} received", _0)]
    OneTarget(usize),

    #[fail(display = "Unrecognized target: {}", _0)]
    UnrecognizedTarget(String),

    #[fail(
        display = "Start and end dates are swapped. Start: {}, end: {}",
        _0, _1
    )]
    DatesSwapped(DateTime<Utc>, DateTime<Utc>),
}
