#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate chrono;
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate rocket;
extern crate rocket_contrib;
extern crate rrinlog_core;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

mod options;
mod api;
mod dao;
mod errors;

use structopt::StructOpt;
use env_logger::{LogBuilder, LogTarget};
use rocket_contrib::Json;
use rocket::State;
use diesel::prelude::*;
use chrono::prelude::*;
use api::*;
use errors::*;

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
fn query(data: Json<Query>, opt: State<options::Opt>) -> Result<Json<QueryResponse>> {
    debug!("Search received: {:?}", data.0);

    let conn =
        SqliteConnection::establish(&opt.db)
        .map_err(|e| Error::from(ErrorKind::DbConn(opt.db.to_owned(), e)))?;

    let rows =
        dao::blog_posts(&conn, &data.range, &opt.ip)
        .map_err(|e| Error::from(ErrorKind::DbQuery("blog posts".to_string(), e)))?;

    let r: Vec<_> = rows.into_iter()
        .map(|x| vec![json!(x.referer), json!(x.views)])
        .collect();

    Ok(Json(QueryResponse(vec![TargetData::Table(create_blog_table(r))])))
}

fn create_blog_table(rows: Vec<Vec<serde_json::value::Value>>) -> api::Table {
    api::Table {
        _type: "table".to_string(),
        columns: vec![
            api::Column {
                text: "article".to_string(),
                _type: "string".to_string(),
            },
            api::Column {
                text: "count".to_string(),
                _type: "number".to_string(),
            },
        ],
        rows: rows,
    }
}

fn init_logging() -> std::result::Result<(), log::SetLoggerError> {
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

fn main() {
    init_logging().expect("Logging to initialize");
    let opt = options::Opt::from_args();
    rocket::ignite()
        .manage(opt)
        .mount("/", routes![index, search, query])
        .launch();
}
