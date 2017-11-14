use diesel::prelude::*;
use diesel::types::*;
use diesel::expression::sql;
use chrono::prelude::*;
use api::*;

#[derive(PartialEq, Debug, Queryable)]
pub struct BlogPost {
    pub referer: String,
    pub views: i32,
}

#[derive(PartialEq, Debug, Queryable)]
pub struct Sites {
    pub ep: i64,
    pub host: String,
    pub views: i32,
}

#[derive(PartialEq, Debug, Queryable)]
pub struct OutboundData {
    pub ep: i64,
    pub views: i32,
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
    let query = sql::<(Text, Integer)>(BLOG_POST_QUERY)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Text, _>(ip);
    LoadDsl::load::<BlogPost>(query, conn)
}

pub fn sites(conn: &SqliteConnection, range: &Range, interval_ms: i32) -> QueryResult<Vec<Sites>> {
    let interval_s = interval_ms / 1000;
    let qs = format!(
        r#"
SELECT (epoch / {}) * {} AS ep,
       host,
       Count(*) AS views
FROM   logs
WHERE  host LIKE "%nbsoftsolutions.com"
       AND epoch >= ?
       AND epoch < ?
GROUP BY epoch / ({}),
         host
"#,
        interval_s,
        interval_ms,
        interval_s
    );

    let query = sql::<(BigInt, Text, Integer)>(&qs)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp());
    LoadDsl::load::<Sites>(query, conn)
}

pub fn outbound_data(
    conn: &SqliteConnection,
    range: &Range,
    ip: &str,
    interval_ms: i32,
) -> QueryResult<Vec<OutboundData>> {
    let interval_s = interval_ms / 1000;
    let qs = format!(
        r#"
SELECT (epoch / {}) * {} AS ep,
       COUNT(*) AS views,
       SUM(body_bytes_sent) as data
FROM   logs
WHERE  epoch >= ?
       AND epoch < ?
       AND remote_addr <> ?
GROUP BY epoch / ({})
ORDER BY ep
"#,
        interval_s,
        interval_ms,
        interval_s
    );

    let query = sql::<(BigInt, Integer, BigInt)>(&qs)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Text, _>(ip);
    LoadDsl::load::<OutboundData>(query, conn)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                referer: "https://nbsoftsolutions.com/blog/designing-a-rest-api-unix-time-vs-iso-8601"
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
                referer: "https://nbsoftsolutions.com/blog/high-performance-unsafe-c-code-is-a-lie-redux"
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
                referer: "https://nbsoftsolutions.com/blog/turning-dropwizard-performance-up-to-eleven"
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

        let result = sites(&conn, &rng, 30000).expect("results");
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

        let result = outbound_data(&conn, &rng, "127.0.0.2", 30000).expect("results");
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
