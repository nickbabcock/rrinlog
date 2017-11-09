use diesel::prelude::*;
use diesel::types::*;
use diesel::expression::sql;
use api::*;

#[derive(PartialEq, Debug, Queryable)]
pub struct BlogPost {
    pub referer: String,
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
