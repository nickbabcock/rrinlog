#![recursion_limit = "128"]
#![cfg_attr(feature = "unstable", feature(test))]

extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate regex;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use std::io;
use std::io::prelude::*;
use diesel::prelude::*;
use structopt::StructOpt;
use error_chain::ChainedError;

mod schema;
mod models;
mod parser;
mod options;

fn main() {
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
    use schema::logs;
    use diesel::result::Error;

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
}

#[cfg(all(feature = "unstable", test))]
mod bench {
    extern crate test;
    use parser;

    #[bench]
    fn bench_parse_nginx(b: &mut test::Bencher) {
        let line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        b.iter(|| {
            assert!(parser::parse_nginx_line(line).is_ok());
        });
    }
}
