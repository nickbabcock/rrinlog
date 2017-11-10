#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate rrinlog_core;
extern crate rocket_contrib;
extern crate rocket;
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
#[macro_use]
extern crate serde_derive;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate chrono;
extern crate env_logger;
#[macro_use]
extern crate log;

mod options;
mod api;
mod dao;

use structopt::StructOpt;
use env_logger::{LogBuilder, LogTarget};
use rocket_contrib::{Json};
use rocket::State;
use diesel::prelude::*;
use chrono::prelude::*;
use api::*;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/search", format = "application/json", data = "<data>")]
fn search(data: Json<Search>) -> Json<SearchResponse> {
    debug!("Search received: {:?}", data.0);
    Json(SearchResponse(vec!["blog_hits".to_string()]))
}

#[post("/query", format = "application/json", data = "<data>")]
fn query(data: Json<Query>, opt: State<options::Opt>) -> Json<QueryResponse> {
    debug!("Search received: {:?}", data.0);
    let conn = SqliteConnection::establish(&opt.db).expect(&format!("Error connecting to {}", opt.db));
    let rows = dao::blog_posts(&conn, &data.range, "67.167.1.208").expect("AA");

    let r: Vec<_> = rows.into_iter().map(|x| vec![json!(x.referer), json!(x.views)]).collect();
    let response = api::Table {
        _type: "table".to_string(),
        columns: vec![
            api::Column { text: "article".to_string(), _type: "string".to_string() },
            api::Column { text: "count".to_string(), _type: "number".to_string() }
        ],
        rows: r
    };

    Json(QueryResponse(vec![TargetData::Table(response)]))
}

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
    rocket::ignite()
        .manage(opt)
        .mount("/", routes![index, search, query]).launch();
}
