use diesel::prelude::*;
use diesel::types::*;
use diesel::expression::sql;
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
    let query = sql::<(Text, Integer)>(&BLOG_POST_QUERY)
        .bind::<BigInt, _>(range.from.timestamp())
        .bind::<BigInt, _>(range.to.timestamp())
        .bind::<Text, _>(ip);
    LoadDsl::load::<BlogPost>(query, conn)
}

pub fn sites(conn: &SqliteConnection, range: &Range, interval_ms: i32) -> QueryResult<Vec<Sites>> {
    let interval_s = interval_ms / 1000;
    let qs = format!(
        r#"
SELECT (epoch / {}) * {} AS nep,
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
