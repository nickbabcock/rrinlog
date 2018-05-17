extern crate actix_web;
extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate rrinlog_core;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate structopt;
extern crate uom;

mod options;
mod api;
mod dao;
mod errors;

use structopt::StructOpt;
use env_logger::{LogBuilder, LogTarget};
use diesel::prelude::*;
use chrono::prelude::*;
use api::*;
use errors::DataError;
use itertools::Itertools;
use failure::Error;
use uom::si::i64::*;
use uom::si::time::{millisecond, second};
use actix_web::{http, server, App, HttpRequest, Json, State};
use actix_web::middleware::Logger;

#[derive(Debug, Clone)]
struct RinState {
    pub db: String,
    pub ip: String,
}

fn index(_req: HttpRequest<RinState>) -> &'static str {
    "Hello world!"
}

fn search(data: Json<Search>) -> Json<SearchResponse> {
    debug!("Search received: {:?}", data);
    Json(SearchResponse(vec![
        "blog_hits".to_string(),
        "sites".to_string(),
        "outbound_data".to_string(),
    ]))
}

fn query(data: (Json<Query>, State<RinState>)) -> Result<Json<QueryResponse>, Error> {
    let (query, opt) = data;
    debug!("Search received: {:?}", query);

    // Acquire SQLite connection on each request. This can be considered inefficient, but since
    // there isn't a roundtrip connection cost the benefit to debugging of never having a stale
    // connection is well worth it.
    let conn =
        SqliteConnection::establish(&opt.db).map_err(|e| DataError::DbConn(opt.db.to_owned(), e))?;

    // Grafana can technically ask for more than one target at once. It can ask for "blog_hits" and
    // "sites" in one request, but we're going to keep it simply and work with only with requests
    // that ask for one set of data.
    let first = query
        .targets
        .first()
        .ok_or_else(|| DataError::OneTarget(query.targets.len()))?;

    // Our code assumes that `from < to` in calculations for vector sizes. Else resizing the vector
    // will underflow and panic
    if query.range.from > query.range.to {
        return Err(DataError::DatesSwapped(query.range.from, query.range.to).into());
    }

    // If grafana gives us an interval that would be less than a whole second, round to a second.
    // Also dimension the primitive, so that it is obvious that we're dealing with seconds. This
    // also protects against grafana giving us a negative interval (which it doesn't, but one
    // should never trust user input)
    let interval: Time = Time::new::<second>(std::cmp::max(query.interval_ms / 1000, 1));

    let result = match first.target.as_str() {
        "blog_hits" => get_blog_posts(&conn, &query, &opt),
        "sites" => get_sites(&conn, &query, interval),
        "outbound_data" => get_outbound(&conn, &query, &opt, interval),
        x => Err(DataError::UnrecognizedTarget(String::from(x)).into()),
    };

    Ok(Json(result?))
}

fn get_sites(
    conn: &SqliteConnection,
    data: &Query,
    interval: Time,
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
            datapoints,
        }));
    }

    Ok(QueryResponse(v))
}

/// The given points slice may have gaps of data between start and end times. This function will
/// fill in those gaps with zeroes.
fn fill_datapoints(range: &Range, interval: Time, points: &[[u64; 2]]) -> Vec<[u64; 2]> {
    let start = Time::new::<second>(range.from.timestamp());
    let end = Time::new::<second>(range.to.timestamp());
    let elements: i64 = i64::from((end - start) / interval);

    let mut data: Vec<u64> = vec![0; elements as usize];
    let time: Vec<u64> = (0..elements)
        .map(|i| (i * interval.get(millisecond) + start.get(millisecond)) as u64)
        .collect();

    for point in points {
        let ptime = Time::new::<millisecond>(point[1] as i64);
        let index = (((ptime - start) / interval).value) as usize;
        data[index] = point[0];
    }

    data.into_iter()
        .zip(time)
        .map(|(data, time)| [data, time])
        .collect()
}

fn get_outbound(
    conn: &SqliteConnection,
    data: &Query,
    opt: &RinState,
    interval: Time,
) -> Result<QueryResponse, Error> {
    let rows = dao::outbound_data(conn, &data.range, &opt.ip, interval)
        .map_err(|e| DataError::DbQuery("outbound data".to_string(), e))?;

    let p: Vec<_> = rows.iter().map(|x| [x.bytes as u64, x.ep as u64]).collect();
    let datapoints = fill_datapoints(&data.range, interval, &p);

    let elem = TargetData::Series(Series {
        target: "outbound_data".to_string(),
        datapoints,
    });

    Ok(QueryResponse(vec![elem]))
}

fn get_blog_posts(
    conn: &SqliteConnection,
    data: &Query,
    opt: &RinState,
) -> Result<QueryResponse, Error> {
    let rows = dao::blog_posts(conn, &data.range, &opt.ip)
        .map_err(|e| DataError::DbQuery("blog posts".to_string(), e))?;

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
        rows,
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

fn create_app(opts: RinState) -> App<RinState> {
    App::with_state(opts)
        .middleware(Logger::default())
        .resource("/", |r| r.f(index))
        .resource("/search", |r| r.method(http::Method::POST).with(search))
        .resource("/query", |r| r.method(http::Method::POST).with(query))
}

fn main() {
    init_logging().expect("Logging to initialize");
    let opts = options::Opt::from_args();
    let (addr, state) = {
        (opts.addr, RinState {
            db: opts.db,
            ip: opts.ip,
        })
    };

    server::new(move || create_app(state.clone()))
        .bind(addr)
        .unwrap()
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestServer;
    use actix_web::HttpMessage;
    use http::header;
    use std::str;

    #[test]
    fn fill_datapoints_empty() {
        let rng = Range {
            from: Utc.ymd(2014, 7, 8).and_hms(9, 10, 11),
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11),
        };
        let actual = fill_datapoints(&rng, Time::new::<second>(30), &Vec::new());

        // In an hour there are 120 - 30 second intervals in an hour
        assert_eq!(actual.len(), 120);

        // Ensure that the gap is interval is upheld
        assert_eq!(actual[1][1] - actual[0][1], 30 * 1000);

        let first_time = Utc.ymd(2014, 7, 8).and_hms(9, 10, 11).timestamp() as u64;
        assert_eq!([0, first_time * 1000], actual[0]);
    }

    #[test]
    fn fill_datapoints_one_filled() {
        let rng = Range {
            from: Utc.ymd(2014, 7, 8).and_hms(9, 10, 11),
            to: Utc.ymd(2014, 7, 8).and_hms(10, 10, 11),
        };

        let fill_time = (Utc.ymd(2014, 7, 8).and_hms(9, 11, 11).timestamp() as u64) * 1000;
        let elem: [u64; 2] = [1, fill_time];

        let actual = fill_datapoints(&rng, Time::new::<second>(30), &vec![elem]);

        // In an hour there are 120 - 30 second intervals in an hour
        assert_eq!(actual.len(), 120);

        // Ensure that the gap is interval is upheld
        assert_eq!(actual[2][1] - actual[1][1], 30 * 1000);
        assert_eq!(actual[3][1] - actual[2][1], 30 * 1000);

        assert_eq!([1, fill_time], actual[2]);
    }

    #[test]
    fn test_root_results() {
        let opt = RinState {
            db: "Some db".to_string(),
            ip: "Some ip".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::GET, "/").finish().unwrap();
        let response = srv.execute(request.send()).unwrap();

        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "text/plain");

        let bytes = srv.execute(response.body()).unwrap();
        assert_eq!(str::from_utf8(&bytes).unwrap(), "Hello world!");
    }

    #[test]
    fn test_search_results() {
        let opt = RinState {
            db: "Some db".to_string(),
            ip: "Some ip".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::POST, "/search")
            .header(header::CONTENT_TYPE, "application/json")
            .body(r#"{"target":"something"}"#)
            .unwrap();
        let response = srv.execute(request.send()).unwrap();

        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "application/json");

        let bytes = srv.execute(response.body()).unwrap();
        assert_eq!(
            str::from_utf8(&bytes).unwrap(),
            r#"["blog_hits","sites","outbound_data"]"#
        );
    }

    #[test]
    fn test_query_blog_results() {
        let opt = RinState {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::POST, "/query")
            .header(header::CONTENT_TYPE, "application/json")
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
            .unwrap();

        let response = srv.execute(request.send()).unwrap();
        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "application/json");
    }

    #[test]
    fn test_query_sites_results() {
        let opt = RinState {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::POST, "/query")
            .header(header::CONTENT_TYPE, "application/json")
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
            .unwrap();

        let response = srv.execute(request.send()).unwrap();
        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "application/json");
    }

    // Should not fail when the interval is less than a second
    #[test]
    fn test_query_sites_tiny_results() {
        let opt = RinState {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::POST, "/query")
            .header(header::CONTENT_TYPE, "application/json")
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
            .unwrap();

        let response = srv.execute(request.send()).unwrap();
        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "application/json");
    }

    #[test]
    fn test_query_outbound_results() {
        let opt = RinState {
            db: "../test-assets/test-access.db".to_string(),
            ip: "127.0.0.2".to_string(),
        };

        let mut srv = TestServer::with_factory(move || create_app(opt.clone()));
        let request = srv.client(http::Method::POST, "/query")
            .header(header::CONTENT_TYPE, "application/json")
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
            .unwrap();

        let response = srv.execute(request.send()).unwrap();
        assert!(response.status().is_success());
        assert_eq!(response.content_type(), "application/json");
    }
}
