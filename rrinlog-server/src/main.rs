#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate chrono;
extern crate diesel;
extern crate dimensioned as dim;
extern crate env_logger;
#[macro_use]
extern crate failure;
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
use errors::DataError;
use itertools::Itertools;
use dim::si;
use failure::Error;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/search", format = "application/json", data = "<data>")]
fn search(data: Json<Search>) -> Json<SearchResponse> {
    debug!("Search received: {:?}", data.0);
    Json(SearchResponse(vec![
        "blog_hits".to_string(),
        "sites".to_string(),
        "outbound_data".to_string(),
    ]))
}

#[post("/query", format = "application/json", data = "<data>")]
fn query(data: Json<Query>, opt: State<options::Opt>) -> Result<Json<QueryResponse>, Error> {
    debug!("Search received: {:?}", data.0);

    // Acquire SQLite connection on each request. This can be considered inefficient, but since
    // there isn't a roundtrip connection cost the benefit to debugging of never having a stale
    // connection is well worth it.
    let conn = SqliteConnection::establish(&opt.db)
        .map_err(|e| DataError::DbConn(opt.db.to_owned(), e))?;

    // Grafana can technically ask for more than one target at once. It can ask for "blog_hits" and
    // "sites" in one request, but we're going to keep it simply and work with only with requests
    // that ask for one set of data.
    let first = data.0
        .targets
        .first()
        .ok_or_else(|| DataError::OneTarget(data.0.targets.len()))?;

    // Our code assumes that `from < to` in calculations for vector sizes. Else resizing the vector
    // will underflow and panic
    if data.0.range.from > data.0.range.to {
        return Err(DataError::DatesSwapped(data.0.range.from, data.0.range.to).into());
    }

    // If grafana gives us an interval that would be less than a whole second, round to a second.
    // Also dimension the primitive, so that it is obvious that we're dealing with seconds. This
    // also protects against grafana giving us a negative interval (which it doesn't, but one
    // should never trust user input)
    let interval = si::Second::new(std::cmp::max(data.0.interval_ms / 1000, 1));

    let result = match first.target.as_str() {
        "blog_hits" => get_blog_posts(&conn, &data, opt),
        "sites" => get_sites(&conn, &data, interval),
        "outbound_data" => get_outbound(&conn, &data, opt, interval),
        x => Err(DataError::UnrecognizedTarget(String::from(x)).into()),
    };

    Ok(Json(result?))
}

fn get_sites(
    conn: &SqliteConnection,
    data: &Query,
    interval: si::Second<i32>,
) -> Result<QueryResponse, Error> {
    let mut rows = dao::sites(conn, &data.range, interval)
        .map_err(|e| DataError::DbQuery("sites".to_string(), e))?;

    // Just like python, in order to group by host, we need to have the vector sorted by host. We
    // include sorting by epoch time as grafana expects time to be sorted
    // TODO: Is there someway to sort by string without having to clone?
    rows.sort_unstable_by_key(|x| (x.host.clone(), x.ep));

    let mut v = Vec::new();
    for (host, points) in &rows.into_iter().group_by(|x| x.host.clone()) {
        // points is a sparse array of the number of views seen at a given epoch ms.
        let p: Vec<_> = points.map(|x| [x.views as u64, x.ep as u64]).collect();
        let datapoints = fill_datapoints(&data.range, interval, &p);

        v.push(TargetData::Series(Series {
            target: host,
            datapoints: datapoints,
        }));
    }

    Ok(QueryResponse(v))
}

/// The given points slice may have gaps of data between start and end times. This function will
/// fill in those gaps with zeroes.
fn fill_datapoints(range: &Range, interval: si::Second<i32>, points: &[[u64; 2]]) -> Vec<[u64; 2]> {
    // Clamp the start and end dates given to us by grafana to the nearest divisible interval
    // that at least is a whole second. Grafana could technically give us an interval that contains
    // a fraction of a second, but I'm not interested in sub-second deliniations for site views
    let interval_s: i64 = i64::from(*(interval / si::Second::new(1)));
    let interval_ms = interval_s * 1000;
    let start = range.from.timestamp() / interval_s * interval_ms;
    let end = range.to.timestamp() / interval_s * interval_ms;

    let mut result: Vec<[u64; 2]> = Vec::new();

    // We know the exact number of elements that we will be returning so pre-allocate that up
    // front. (end - start) / step
    result.reserve_exact(((end - start) / i64::from(interval_ms)) as usize);

    // Copy the values from the given slice and fill the gaps with zeroes
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

fn get_outbound(
    conn: &SqliteConnection,
    data: &Query,
    opt: State<options::Opt>,
    interval: si::Second<i32>,
) -> Result<QueryResponse, Error> {
    let rows = dao::outbound_data(conn, &data.range, &opt.ip, interval).map_err(|e| {
        DataError::DbQuery("outbound data".to_string(), e)
    })?;

    let p: Vec<_> = rows.iter().map(|x| [x.bytes as u64, x.ep as u64]).collect();
    let datapoints = fill_datapoints(&data.range, interval, &p);

    let elem = TargetData::Series(Series {
        target: "outbound_data".to_string(),
        datapoints: datapoints,
    });

    Ok(QueryResponse(vec![elem]))
}

fn get_blog_posts(
    conn: &SqliteConnection,
    data: &Query,
    opt: State<options::Opt>,
) -> Result<QueryResponse, Error> {
    let rows = dao::blog_posts(conn, &data.range, &opt.ip).map_err(|e| {
        DataError::DbQuery("blog posts".to_string(), e)
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

fn rocket(opt: options::Opt) -> rocket::Rocket {
    rocket::ignite()
        .manage(opt)
        .mount("/", routes![index, search, query])
}

fn main() {
    init_logging().expect("Logging to initialize");
    rocket(options::Opt::from_args()).launch();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::Client;
    use rocket::http::{ContentType, Status};

    #[test]
    fn fill_datapoints_empty() {
        let rng = Range {
            from: Utc.ymd(2014, 7, 8).and_hms(9, 10, 11),
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11),
        };
        let actual = fill_datapoints(&rng, si::Second::new(30), &Vec::new());

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
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11),
        };

        let fill_time = (Utc.ymd(2014, 7, 8).and_hms(9, 11, 0).timestamp() as u64) * 1000;
        let elem: [u64; 2] = [1, fill_time];

        let actual = fill_datapoints(&rng, si::Second::new(30), &vec![elem]);

        // In an hour there are 120 - 30 second intervals in an hour
        assert_eq!(actual.len(), 120);

        // Ensure that the gap is interval is upheld
        assert_eq!(actual[2][1] - actual[1][1], 30 * 1000);
        assert_eq!(actual[3][1] - actual[2][1], 30 * 1000);

        assert_eq!([1, fill_time], actual[2]);
    }

    #[test]
    fn test_root_results() {
        let opt = options::Opt {
            db: "Some db".to_string(),
            ip: "Some ip".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let mut response = client.get("/").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::Plain));
        assert_eq!(response.body_string(), Some("Hello, world!".into()));
    }

    #[test]
    fn test_search_results() {
        let opt = options::Opt {
            db: "Some db".to_string(),
            ip: "Some ip".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let mut response = client
            .post("/search")
            .body(r#"{"target":"something"}"#)
            .header(ContentType::JSON)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::JSON));
        assert_eq!(
            response.body_string(),
            Some(r#"["blog_hits","sites","outbound_data"]"#.into())
        );
    }

    #[test]
    fn test_query_blog_results() {
        let opt = options::Opt {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let response = client
            .post("/query")
            .body(
                r#"
{
  "panelId": 1,
  "range": {
    "from": "2017-11-14T13:00:00.866Z",
    "to": "2017-11-14T14:00:00.866Z",
    "raw": {
      "from": "now-1h",
      "to": "now"
    }
  },
  "rangeRaw": {
    "from": "now-1h",
    "to": "now"
  },
  "interval": "30s",
  "intervalMs": 30000,
  "targets": [
     { "target": "blog_hits", "refId": "A", "type": "table" }
  ],
  "format": "json",
  "maxDataPoints": 550
}
"#,
            )
            .header(ContentType::JSON)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::JSON));
    }

    #[test]
    fn test_query_sites_results() {
        let opt = options::Opt {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let response = client
            .post("/query")
            .body(
                r#"
{
  "panelId": 1,
  "range": {
    "from": "2017-11-14T13:00:00.866Z",
    "to": "2017-11-14T14:00:00.866Z",
    "raw": {
      "from": "now-1h",
      "to": "now"
    }
  },
  "rangeRaw": {
    "from": "now-1h",
    "to": "now"
  },
  "interval": "30s",
  "intervalMs": 30000,
  "targets": [
     { "target": "sites", "refId": "A", "type": "table" }
  ],
  "format": "json",
  "maxDataPoints": 550
}
"#,
            )
            .header(ContentType::JSON)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::JSON));
    }

    // Should not fail when the interval is less than a second
    #[test]
    fn test_query_sites_tiny_results() {
        let opt = options::Opt {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let response = client
            .post("/query")
            .body(
                r#"
{
  "panelId": 1,
  "range": {
    "from": "2017-11-14T13:00:00.866Z",
    "to": "2017-11-14T14:00:00.866Z",
    "raw": {
      "from": "now-1h",
      "to": "now"
    }
  },
  "rangeRaw": {
    "from": "now-1h",
    "to": "now"
  },
  "interval": "50ms",
  "intervalMs": 50,
  "targets": [
     { "target": "sites", "refId": "A", "type": "table" }
  ],
  "format": "json",
  "maxDataPoints": 550
}
"#,
            )
            .header(ContentType::JSON)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::JSON));
    }

    #[test]
    fn test_query_outbound_results() {
        let opt = options::Opt {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let client = Client::new(rocket(opt)).expect("valid rocket instance");
        let response = client
            .post("/query")
            .body(
                r#"
{
  "panelId": 1,
  "range": {
    "from": "2017-11-14T13:00:00.866Z",
    "to": "2017-11-14T14:00:00.866Z",
    "raw": {
      "from": "now-1h",
      "to": "now"
    }
  },
  "rangeRaw": {
    "from": "now-1h",
    "to": "now"
  },
  "interval": "30s",
  "intervalMs": 30000,
  "targets": [
     { "target": "outbound_data", "refId": "A", "type": "timeserie" }
  ],
  "format": "json",
  "maxDataPoints": 550
}
"#,
            )
            .header(ContentType::JSON)
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::JSON));
    }
}
