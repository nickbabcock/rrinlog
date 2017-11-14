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
    Json(SearchResponse(vec!["blog_hits".to_string(), "sites".to_string()]))
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

    let result = match first?.target.as_str() {
        "blog_hits" => get_blog_posts(&conn, &data, opt),
        "sites" => get_sites(&conn, &data),
        x => Err(Error::from(ErrorKind::UnrecognizedTarget(String::from(x)))),
    };

    Ok(Json(result?))
}

fn get_sites(conn: &SqliteConnection, data: &Query) -> Result<QueryResponse> {
    let mut rows = dao::sites(conn, &data.range, data.interval_ms)
        .map_err(|e| Error::from(ErrorKind::DbQuery("sites".to_string(), e)))?;

    // Just like python, in order to group by host, we need to have the
    // vector sorted by host.
    // TODO: Is there someway to sort by string without having to clone?
    rows.sort_unstable_by_key(|x| (x.host.clone(), x.ep));

    let mut v = Vec::new();
    for (host, points) in &rows.into_iter().group_by(|x| x.host.clone()) {
        v.push(TargetData::Series(Series {
            target: host,
            datapoints: fill_datapoints(&data.range, data.interval_ms, points.map(|x| [x.views as u64, x.ep as u64]).collect()),
        }));
    }

    Ok(QueryResponse(v))
}

fn fill_datapoints(range: &Range, interval_ms: i32, points: Vec<[u64; 2]>) -> Vec<[u64; 2]> {
    let interval_s = i64::from(interval_ms / 1000);
    let start = range.from.timestamp() / interval_s * i64::from(interval_ms);
    let end = range.to.timestamp() / interval_s * i64::from(interval_ms);

    let mut result: Vec<[u64; 2]> = Vec::new();

    // We know the exact number of elements that we will be returning so pre-allocate that up
    // front.
    result.reserve_exact(((end - start) / i64::from(interval_ms)) as usize);
    let mut cur_ind = 0;

    let mut i = start;
    while i < end {
        if cur_ind >= points.len() || points[cur_ind][1] > (i as u64) {
            result.push([0, i as u64]);
        } else {
            result.push(points[cur_ind]);
            cur_ind += 1;
        }
        i += i64::from(interval_ms);
    }
    result
}

fn get_blog_posts(
    conn: &SqliteConnection,
    data: &Query,
    opt: State<options::Opt>,
) -> Result<QueryResponse> {
    let rows = dao::blog_posts(conn, &data.range, &opt.ip).map_err(|e| {
        Error::from(ErrorKind::DbQuery("blog posts".to_string(), e))
    })?;

    // Grafana expects rows to contain heterogeneous values in the same order as the table columns.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_datapoints_empty() {
        let rng = Range {
            from: Utc.ymd(2014, 7, 8).and_hms(9, 10, 11),
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11)
        };
        let actual = fill_datapoints(&rng, 30 * 1000, Vec::new());

        // In an hour there are 120 - 30 second intervals in an hour
        assert_eq!(actual.len(), 120);

        // Ensure that the gap is interval is upheld
        assert_eq!(actual[1][1] - actual[0][1], 30 * 1000);

        let first_time = Utc.ymd(2014, 7, 8).and_hms(9, 10, 0).timestamp() as u64;
        assert_eq!([0, first_time * 1000], actual[0]);
    }

    #[test]
    fn fill_datapoints_one_filled() {
        let rng = Range {
            from: Utc.ymd(2014, 7, 8).and_hms(9, 10, 11),
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11)
        };

        let fill_time = (Utc.ymd(2014, 7, 8).and_hms(9, 11, 0).timestamp() as u64) * 1000;
        let elem: [u64; 2] = [1, fill_time];

        let actual = fill_datapoints(&rng, 30 * 1000, vec![elem]);

        // In an hour there are 120 - 30 second intervals in an hour
        assert_eq!(actual.len(), 120);

        // Ensure that the gap is interval is upheld
        assert_eq!(actual[2][1] - actual[1][1], 30 * 1000);
        assert_eq!(actual[3][1] - actual[2][1], 30 * 1000);

        assert_eq!([1, fill_time], actual[2]);
    }
}
