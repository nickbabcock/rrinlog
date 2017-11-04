#![recursion_limit="128"]

extern crate regex;
extern crate chrono;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;

use std::io;
use std::io::prelude::*;
use diesel::prelude::*;

mod schema;
mod models;
mod parser;

pub fn establish_connection() -> SqliteConnection {
    let database_url = "logs.db";
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

fn main() {
	let stdin = io::stdin();
    let mut buffer: Vec<String> = Vec::new();
	for line in stdin.lock().lines() {
        buffer.push(line.unwrap());
        if buffer.len() == 10 {
            insert_buffer(&buffer);
            buffer.clear();
        }
	}

    insert_buffer(&buffer)
}

fn insert_buffer(buffer: &Vec<String>) {
    for l in buffer {
        println!("{:?}", parser::parse_nginx_line(l.as_str()));
    }
}
