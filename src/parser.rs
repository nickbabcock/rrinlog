use chrono::prelude::*;
use regex::Regex;

#[derive(Fail, Debug, PartialEq, Clone)]
pub enum ParseError {
    #[fail(display = "Text did not match regex `{}`", _0)]
    NoMatch(String),
    #[fail(display = "Text could not be parsed into date `{}`", _0)]
    InvalidDate(String),
}

use models::*;

pub fn parse_nginx_line(text: &str) -> Result<NewLog, ParseError> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r#"(?x)
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
        HTTP/(?P<version>[^\s]+)"
        \s
        (?P<status>[^\s]+)
        \s
        (?P<body_bytes_sent>[^\s]+)
        \s
        "(?P<referer>[^"]*)"
        \s
        "(?P<user_agent>[^"]*)"
        \s
        "(?P<host>[^"]+)""#
        )
        .unwrap();
    }

    if let Some(caps) = RE.captures(text) {
        Ok(NewLog {
            epoch: parse_date(caps.name("time_local").unwrap().as_str())?,
            remote_addr: Some(caps.name("remote_addr").unwrap().as_str()),
            remote_user: Some(caps.name("remote_user").unwrap().as_str()),
            status: caps.name("status").unwrap().as_str().parse::<i32>().ok(),
            method: Some(caps.name("method").unwrap().as_str()),
            path: Some(caps.name("path").unwrap().as_str()),
            version: Some(caps.name("version").unwrap().as_str()),
            body_bytes_sent: caps
                .name("body_bytes_sent")
                .unwrap()
                .as_str()
                .parse::<i32>()
                .ok(),
            referer: Some(caps.name("referer").unwrap().as_str()),
            user_agent: Some(caps.name("user_agent").unwrap().as_str()),
            host: caps.name("host").unwrap().as_str(),
        })
    } else {
        Err(ParseError::NoMatch(String::from(text)))
    }
}

pub fn parse_date(text: &str) -> Result<i64, ParseError> {
    if let Ok(dt) = DateTime::parse_from_str(text, "%d/%b/%Y:%H:%M:%S %z") {
        Ok(dt.timestamp())
    } else {
        Err(ParseError::InvalidDate(String::from(text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        let expected = FixedOffset::west(5 * 3600)
            .ymd(2017, 11, 3)
            .and_hms(6, 49, 45);
        let actual = parse_date("03/Nov/2017:06:49:45 -0500").unwrap();
        assert_eq!(expected.timestamp(), actual);
    }

    #[test]
    fn test_parse_bad_date() {
        let actual = parse_date("2017-12-01");
        assert!(actual.is_err());
        let err: ParseError = actual.unwrap_err();
        let s = format!("{}", err);
        assert_eq!("Text could not be parsed into date `2017-12-01`", s);
    }

    #[test]
    fn test_parse_nginx() {
        let line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        let actual = parse_nginx_line(line).unwrap();
        assert_eq!(
            NewLog {
                epoch: 1509818735,
                remote_addr: Some("127.0.0.1"),
                remote_user: Some("-"),
                status: Some(200),
                method: Some("GET"),
                path: Some("/js/embed.min.js"),
                version: Some("2.0"),
                body_bytes_sent: Some(20480),
                referer: Some(
                    "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana"
                ),
                user_agent: Some(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36"
                ),
                host: "comments.nbsoftsolutions.com",
            },
            actual
        )
    }
}
