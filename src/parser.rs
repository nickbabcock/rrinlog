use regex::Regex;

mod errors {
    error_chain!{
        errors {
            NoMatch(text: String) {
                description("Text did not match regex")
                display("Text did not match regex `{}`", text)
            }
        }
    }
}

use models::*;
use self::errors::*;

pub fn parse_nginx_line(text: &str) -> Result<NewLog> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
        (?P<remote_addr>[^\s]+)
        \s-\s
        (?P<remote_user>[^\s]*)
        \s\[
        (?P<time_local>[^\]]+)
        \]\s"
        (?P<method>[^\s]+)
        \s
        (?P<path>[^\s]*)
        \s
        HTTP/(?P<version>[^\s]+)
        \s
        (?P<status>[^\s]+)
        \s
        (?P<body_bytes_sent>[^\s]+)
        \s
        "(?P<referer>[^"]*)"
        \s
        "(?P<user_agent>[^"]*)"
        \s
        "(?P<host>[^"]+)""#).unwrap();
    }

    if let Some(caps) = RE.captures(text) {

        Ok(NewLog {
            epoch: 0,
            remote_addr: Some(&caps["remote_addr"]),
            remote_user: Some(&caps["remote_user"]),
            status: Some(200),
            method: Some(&caps["method"]),
            path: Some(&caps["path"]),
            version: Some(&caps["version"]),
            body_bytes_sent: Some(200),
            referer: Some(&caps["refer"]),
            user_agent: Some(&caps["user_agent"]),
            host: &caps["host"]
        })
    } else {
        Err(Error::from(ErrorKind::NoMatch(String::from(text))))
    }
}
