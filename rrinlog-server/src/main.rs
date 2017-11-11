#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate chrono;
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate itertools;
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
use itertools::Itertools;

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

    // Acquire SQLite connection on each request. This can be considered inefficient, but since
    // there isn't a roundtrip connection cost the benefit to debugging of never having a stale
    // connection is well worth it.
    let conn = SqliteConnection::establish(&opt.db)
        .map_err(|e| Error::from(ErrorKind::DbConn(opt.db.to_owned(), e)))?;

    // Grafana can technically ask for more than one target at once. It can ask for "blog_hits" and
    // "sites" in one request, but we're going to keep it simply and work with only with requests
    // that ask for one set of data.
    let first: errors::Result<&api::Target> = data.0
        .targets
        .first()
        .ok_or_else(|| Error::from(ErrorKind::OneTarget(data.0.targets.len())));



    let result: errors::Result<QueryResponse> = match first?.target.as_str() {
        "blog_hits" => get_blog_posts(&conn, &data, opt),
        "sites" => get_sites(&conn, &data),
        x => Err(Error::from(ErrorKind::UnrecognizedTarget(String::from(x)))),
    };


    Ok(Json(result?))
}

fn get_sites(conn: &SqliteConnection, data: &Query) -> Result<QueryResponse> {
    let mut rows = dao::sites(&conn, &data.range, data.interval_ms)
        .map_err(|e| Error::from(ErrorKind::DbQuery("sites".to_string(), e)))?;
    rows.sort_unstable_by_key(|x| x.host.clone());

    let mut v = Vec::new();
    for (host, points) in &rows.into_iter().group_by(|x| x.host.clone()) {
        v.push(TargetData::Series(Series {
            target: host,
            datapoints: points.map(|x| [x.views as u64, x.ep as u64]).collect(),
        }));
    }


    Ok(QueryResponse(v))
}

fn get_blog_posts(
    conn: &SqliteConnection,
    data: &Query,
    opt: State<options::Opt>,
) -> Result<QueryResponse> {
    let rows = dao::blog_posts(&conn, &data.range, &opt.ip).map_err(|e| {
        Error::from(ErrorKind::DbQuery("blog posts".to_string(), e))
    })?;

    let r: Vec<_> = rows.into_iter()
        .map(|x| vec![json!(x.referer), json!(x.views)])
        .collect();

    Ok(QueryResponse(vec![TargetData::Table(create_blog_table(r))]))
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
