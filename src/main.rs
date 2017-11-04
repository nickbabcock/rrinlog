#![recursion_limit="128"]

extern crate regex;
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
	for line in stdin.lock().lines() {
		println!("{}", line.unwrap());
	}
}
