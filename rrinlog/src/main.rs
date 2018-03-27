#![recursion_limit = "128"]

extern crate chrono;
extern crate diesel;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate log;
extern crate rrinlog_core;
#[macro_use]
extern crate structopt;

use std::io;
use std::io::prelude::*;
use diesel::prelude::*;
use structopt::StructOpt;
use chrono::prelude::*;
use env_logger::{LogBuilder, LogTarget};
use rrinlog_core::parser;

mod options;

fn main() {
    init_logging().expect("Logging to initialize");

    let opt = options::Opt::from_args();
    if opt.dry_run {
        dry_run();
    } else {
        persist_logs(opt.buffer, &opt.db);
    }
}

fn init_logging() -> Result<(), log::SetLoggerError> {
    LogBuilder::new()
        .format(|record| {
            format!(
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .parse(&std::env::var("RUST_LOG").unwrap_or_default())
        .target(LogTarget::Stdout)
        .init()
}

fn persist_logs(threshold: usize, db: &str) {
    let conn = SqliteConnection::establish(db).expect(&format!("Error connecting to {}", db));
    let mut buffer: Vec<String> = Vec::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        buffer.push(line.unwrap());
        if buffer.len() >= threshold {
            insert_buffer(&conn, &mut buffer);
        }
    }

    insert_buffer(&conn, &mut buffer)
}

fn dry_run() {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match parser::parse_nginx_line(line.unwrap().as_str()) {
            // Both Ok and Err branches halt writing if the line can't be ouput.
            // For instance, this occurs when rrinlog output is piped to head
            Ok(log) => if writeln!(&mut handle, "line: {}", log).is_err() {
                break;
            },
            Err(ref e) => if writeln!(&mut handle, "error: {}", e).is_err() {
                break;
            },
        }
    }
}

/// If SQLite transaction successfully acquired, `insert_buffer` will drain the provided buffer of
/// log lines even if the line can't be parsed or inserted.
fn insert_buffer(conn: &SqliteConnection, buffer: &mut Vec<String>) {
    use rrinlog_core::schema::logs;
    use diesel::result::Error;

    let start = Utc::now();
    let init_len = buffer.len();
    let mut success = 0;

    // Open a transaction and run a bunch of individual inserts. The transaction is important as
    // inserts grouped under a single transaction are faster as there are less locks involved. See
    // diesel pull request for further info: https://github.com/diesel-rs/diesel/pull/1183
    let trans = conn.transaction::<_, Error, _>(|| {
        for l in buffer.drain(..) {
            match parser::parse_nginx_line(l.as_str()) {
                Ok(ng) => {
                    // If we can't insert our parsed log then our schema must not be representative
                    // of the data. The error shouldn't be a sqlite write conflict as that is
                    // checked at the transaction level, but since I'm not all-knowing I won't
                    // assume the cause of the error. Instead of panicking, discard the line and
                    // log the error.
                    if let Err(ref e) = diesel::insert_into(logs::table).values(&ng).execute(conn) {
                        error!("Insertion error: {}", e)
                    } else {
                        success += 1;
                    }
                }

                // If we can't parse a line, yeah that sucks but it's bound to happen so discard
                // the line after it's logged for the attentive sysadmin
                Err(ref e) => error!("Parsing error: {}", e),
            }
        }

        // There are no early returns, so we should always want to commit the transaction. In the
        // interest of conserving resources and stability, log lines not able to be parsed or
        // inserted are discarded.
        Ok(())
    });

    // If SQLite is unable to acquire needed locks for the write to succeed (for instance if WAL
    // mode is off and there is a reader), the transaction will error. Don't panic as rrinlog is
    // supposed to be a long lived application. We'll simply wait for the next line to try again
    if let Err(ref e) = trans {
        error!("Unable to complete transaction: {}", e);
        return;
    }

    let end = Utc::now();
    let dur = end.signed_duration_since(start);
    info!(
        "Parsing and inserting {} out of {} records took {}us",
        success,
        init_len,
        dur.num_microseconds().unwrap()
    );
}


#[cfg(test)]
mod tests {
    extern crate assert_cli;
    extern crate tempdir;
    extern crate environment;

    use std::path::PathBuf;
    use std::env;

    #[test]
    fn test_dry_run_empty_input() {
        assert_cli::Assert::main_binary()
            .with_args(&["--dry-run"])
            .succeeds()
            .unwrap();
    }

    #[test]
    fn test_dry_run_with_input() {
        let fail_line = "Cats are alright"; 
        let success_line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        assert_cli::Assert::main_binary()
            .with_args(&["--dry-run"])
            .stdin(&format!("{}\n{}", fail_line, success_line))
            .succeeds()
            .stdout().contains("line: ")
            .stdout().contains("error: ")
            .unwrap();
    }

    #[test]
    fn run_db_test() {
        let tmp_dir = tempdir::TempDir::new("rrinlog").unwrap();
        let tmp_path = tmp_dir.path().join("logs.db");
        let tmp = tmp_path.to_str().unwrap();
        let migration_dir = PathBuf::from(r"../migrations");
        let migration = migration_dir.to_str().unwrap();
        println!("Current dir: {:?}", env::current_dir());
        assert_cli::Assert::command(&["diesel"])
            .with_args(&["setup", "--migration-dir", migration, "--database-url", tmp])
            .succeeds()
            .unwrap();

        let fail_line = "Cats are alright"; 
        let success_line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        assert_cli::Assert::main_binary()
            .with_env(environment::Environment::inherit().insert("RUST_LOG", "INFO"))
            .with_args(&["--buffer", "1", "--db", tmp])
            .stdin(&format!("{}\n{}", fail_line, success_line))
            .succeeds()
            .stdout().contains("Parsing and inserting 0 out of 1")
            .stdout().contains("Parsing and inserting 1 out of 1")
            .unwrap();
    }
}
