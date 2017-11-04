use schema::logs;

#[derive(Queryable)]
pub struct Log {
    pub epoch: i64,
    pub remote_addr: Option<String>,
    pub remote_user: Option<String>,
    pub status: Option<i32>,
    pub method: Option<String>,
    pub path: Option<String>,
    pub version: Option<String>,
    pub body_bytes_send: Option<i64>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub host: String,
}


#[derive(Insertable)]
#[table_name="logs"]
pub struct NewLog<'a> {
    pub epoch: i64,
    pub remote_addr: Option<&'a str>,
    pub remote_user: Option<&'a str>,
    pub status: Option<i32>,
    pub method: Option<&'a str>,
    pub path: Option<&'a str>,
    pub version: Option<&'a str>,
    pub body_bytes_sent: Option<i32>,
    pub referer: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub host: &'a str,
}

