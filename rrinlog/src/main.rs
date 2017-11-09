#![recursion_limit = "128"]
#![cfg_attr(feature = "unstable", feature(test))]

extern crate chrono;
extern crate diesel;
extern crate env_logger;
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate rrinlog_core;

use std::io;
use std::io::prelude::*;
use diesel::prelude::*;
use structopt::StructOpt;
use error_chain::ChainedError;
use chrono::prelude::*;
use env_logger::{LogBuilder, LogTarget};
use rrinlog_core::parser;

mod options;

fn main() {
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
        .expect("Logging to initialize");

    let opt = options::Opt::from_args();
    if opt.dry_run {
        dry_run();
    } else {
        persist_logs(opt.buffer, &opt.db);
    }
}

fn persist_logs(threshold: usize, db: &str) {
    let conn = SqliteConnection::establish(db).expect(&format!("Error connecting to {}", db));
    let mut buffer: Vec<String> = Vec::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        buffer.push(line.unwrap());
        if buffer.len() == threshold {
            insert_buffer(&conn, &buffer);
            buffer.clear();
        }
    }

    insert_buffer(&conn, &buffer)
}

fn dry_run() {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match parser::parse_nginx_line(line.unwrap().as_str()) {
            Ok(log) => if writeln!(&mut handle, "line: {}", log).is_err() {
                break;
            },
            Err(ref e) => if writeln!(&mut handle, "error: {}", e.display_chain()).is_err() {
                break;
            },
        }
    }
}

fn insert_buffer(conn: &SqliteConnection, buffer: &[String]) {
    use rrinlog_core::schema::logs;
    use diesel::result::Error;

    let start = Utc::now();
    conn.transaction::<_, Error, _>(|| {
        for l in buffer {
            match parser::parse_nginx_line(l.as_str()) {
                Ok(ng) => {
                    diesel::insert(&ng)
                        .into(logs::table)
                        .execute(conn)
                        .expect("to insert records");
                }
                Err(ref e) => error!("{}", e.display_chain()),
            }
        }
        Ok(())
    }).expect("to complete transaction");
    let end = Utc::now();
    let dur = end.signed_duration_since(start);
    info!(
        "Parsing and inserting {} records took {}us",
        buffer.len(),
        dur.num_microseconds().unwrap()
    );
}
