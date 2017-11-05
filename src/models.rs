use schema::logs;
use std::fmt;

#[derive(Debug, Queryable, PartialEq)]
pub struct Log {
    pub epoch: i64,
    pub remote_addr: Option<String>,
    pub remote_user: Option<String>,
    pub status: Option<i32>,
    pub method: Option<String>,
    pub path: Option<String>,
    pub version: Option<String>,
    pub body_bytes_send: Option<i32>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub host: String,
}


#[derive(Debug, Insertable, PartialEq)]
#[table_name = "logs"]
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

impl<'a> fmt::Display for NewLog<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} {} {} {} {} {}",
            self.epoch,
            self.remote_addr.unwrap_or("NA"),
            self.remote_user.unwrap_or("NA"),
            self.status.unwrap_or(200),
            self.method.unwrap_or("NA"),
            self.path.unwrap_or("NA"),
            self.version.unwrap_or("NA"),
            self.body_bytes_sent.unwrap_or(0),
            self.referer.unwrap_or("NA"),
            self.user_agent.unwrap_or("NA"),
            self.host
        )
    }
}
