use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Text};
use diesel::sql_query;
use api::*;
use uom::si::i64::*;
use uom::si::time::second;

#[derive(PartialEq, Debug, QueryableByName)]
pub struct BlogPost {
    #[sql_type = "Text"]
    pub referer: String,
    #[sql_type = "Integer"]
    pub views: i32,
}

#[derive(PartialEq, Debug, QueryableByName)]
pub struct Sites {
    #[sql_type = "BigInt"]
    pub ep: i64,
    #[sql_type = "Text"]
    pub host: String,
    #[sql_type = "Integer"]
    pub views: i32,
}

#[derive(PartialEq, Debug, QueryableByName)]
pub struct OutboundData {
    #[sql_type = "BigInt"]
    pub ep: i64,
    #[sql_type = "Integer"]
    pub views: i32,
    #[sql_type = "BigInt"]
    #[column_name = "data"]
    pub bytes: i64,
}

static BLOG_POST_QUERY: &'static str = r#"
SELECT referer,
       Count(*) AS views
FROM   logs
WHERE  host = 'comments.nbsoftsolutions.com'
       AND method = 'GET'
       AND path <> '/js/embed.min.js'
       AND epoch >= ?
       AND epoch < ?
       AND referer <> '-'
       AND remote_addr <> ?
GROUP  BY referer
ORDER  BY views DESC
"#;

pub fn blog_posts(conn: &SqliteConnection, range: &Range, ip: &str) -> QueryResult<Vec<BlogPost>> {
    sql_query(BLOG_POST_QUERY)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Text, _>(ip)
        .load(conn)
}

pub fn sites(conn: &SqliteConnection, range: &Range, interval: Time) -> QueryResult<Vec<Sites>> {
    let qs = r#"
SELECT (epoch / ?) * ? * 1000 AS ep,
       host,
       Count(*) AS views
FROM   logs
WHERE  host LIKE "%nbsoftsolutions.com"
       AND epoch >= ?
       AND epoch < ?
GROUP BY epoch / ?,
         host
"#;

    sql_query(qs)
        .bind::<Integer, _>(interval.get(second) as i32)
        .bind::<Integer, _>(interval.get(second) as i32)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Integer, _>(interval.get(second) as i32)
        .load(conn)
}

pub fn outbound_data(
    conn: &SqliteConnection,
    range: &Range,
    ip: &str,
    interval: Time,
) -> QueryResult<Vec<OutboundData>> {
    let qs = format!(
        r#"
SELECT (epoch / {}) * {} * 1000 AS ep,
       COUNT(*) AS views,
       SUM(body_bytes_sent) as data
FROM   logs
WHERE  epoch >= ?
       AND epoch < ?
       AND remote_addr <> ?
GROUP BY epoch / ({})
ORDER BY ep
"#,
        interval.get(second),
        interval.get(second),
        interval.get(second)
    );

    sql_query(qs)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Text, _>(ip)
        .load(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_blog_posts() {
        let conn =
            SqliteConnection::establish("../test-assets/test-access.db").expect("To open db");
        let rng = Range {
            from: Utc.ymd(2017, 11, 14).and_hms(13, 0, 0),
            to: Utc.ymd(2017, 11, 14).and_hms(14, 0, 0),
        };

        let result = blog_posts(&conn, &rng, "127.0.0.2").expect("results");
        assert_eq!(8, result.len());

        assert_eq!(
            result[0],
            BlogPost {
                referer: "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana"
                    .to_string(),
                views: 6,
            }
        );
        assert_eq!(
            result[1],
            BlogPost {
                referer: "https://nbsoftsolutions.com/blog/getting-started-with-dropwizard-testing"
                    .to_string(),
                views: 3,
            }
        );
        assert_eq!(
            result[2],
            BlogPost {
                referer:
                    "https://nbsoftsolutions.com/blog/designing-a-rest-api-unix-time-vs-iso-8601"
                        .to_string(),
                views: 2,
            }
        );
        assert_eq!(
            result[3],
            BlogPost {
                referer: "https://nbsoftsolutions.com/blog/high-performance-unsafe-c-code-is-a-lie"
                    .to_string(),
                views: 1,
            }
        );
        assert_eq!(
            result[4],
            BlogPost {
                referer:
                    "https://nbsoftsolutions.com/blog/high-performance-unsafe-c-code-is-a-lie-redux"
                        .to_string(),
                views: 1,
            }
        );
        assert_eq!(
            result[5],
            BlogPost {
                referer: "https://nbsoftsolutions.com/blog/know-thy-threadpool-a-worked-example-with-dropwizard"
                    .to_string(),
                views: 1,
            }
        );
        assert_eq!(
            result[6],
            BlogPost {
                referer: "https://nbsoftsolutions.com/blog/linux-virtualization-with-a-mounted-windows-share-on-client-hyper-v"
                    .to_string(),
                views: 1,
            }
        );
        assert_eq!(
            result[7],
            BlogPost {
                referer:
                    "https://nbsoftsolutions.com/blog/turning-dropwizard-performance-up-to-eleven"
                        .to_string(),
                views: 1,
            }
        );
    }

    #[test]
    fn test_sites() {
        let conn =
            SqliteConnection::establish("../test-assets/test-access.db").expect("To open db");
        let rng = Range {
            from: Utc.ymd(2017, 11, 14).and_hms(13, 0, 3),
            to: Utc.ymd(2017, 11, 14).and_hms(14, 0, 3),
        };

        let result = sites(&conn, &rng, Time::new::<second>(30)).expect("results");
        assert_eq!(18, result.len());
        assert_eq!(
            Sites {
                ep: 1510664490000,
                host: "comments.nbsoftsolutions.com".to_string(),
                views: 5,
            },
            result[0]
        );
    }

    #[test]
    fn test_outbound_data() {
        let conn =
            SqliteConnection::establish("../test-assets/test-access.db").expect("To open db");
        let rng = Range {
            from: Utc.ymd(2017, 11, 14).and_hms(13, 0, 3),
            to: Utc.ymd(2017, 11, 14).and_hms(14, 0, 3),
        };

        let result =
            outbound_data(&conn, &rng, "127.0.0.2", Time::new::<second>(30)).expect("results");
        assert_eq!(18, result.len());
        assert_eq!(
            OutboundData {
                ep: 1510664490000,
                views: 5,
                bytes: 1782,
            },
            result[0]
        );
    }
}
