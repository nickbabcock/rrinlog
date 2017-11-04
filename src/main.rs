#![recursion_limit="128"]

extern crate chrono;
extern crate structopt;
extern crate regex;
extern crate env_logger;
#[macro_use] extern crate log;
#[macro_use] extern crate structopt_derive;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;

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
    let conn = SqliteConnection::establish(db)
        .expect(&format!("Error connecting to {}", db));
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
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match parser::parse_nginx_line(line.unwrap().as_str()) {
            Ok(log) => {
                println!("line: {:?}", log);
            }
            Err(ref e) => {
                println!("error: {:?}", e.display_chain());
            }
        }
    }
}

fn insert_buffer(conn: &SqliteConnection, buffer: &[String]) {
    use schema::logs;

    let mut a: Vec<_> = Vec::new();
    for l in buffer {
        match parser::parse_nginx_line(l.as_str()) {
            Ok(ng) => a.push(ng),
            Err(ref e) => error!("{}", e.display_chain())
        }
    }

    diesel::insert(&a).into(logs::table)
        .execute(conn)
        .expect("Error saving new logs");
}
