use diesel::result::ConnectionError;
use diesel::result::Error as DsError;

error_chain!{
    errors {
        DbConn(db: String, ds: ConnectionError) {
            description("Unable to connect to database")
            display("Unable to connect to database `{}` {}", db, ds)
        }

        DbQuery(desc: String, err: DsError) {
            description("Unable to execute query")
            display("Unable to execute query: {}: {}", desc, err)
        }

        OneTarget(targets: usize) {
            description("One target expected")
            display("One target expected: {} received", targets)
        }

        UnrecognizedTarget(target: String) {
            description("Unrecognized target")
            display("Unrecognized target: {}", target)
        }
    }
}
