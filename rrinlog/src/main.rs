#![recursion_limit = "128"]

extern crate chrono;
extern crate diesel;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate log;
extern crate rrinlog_core;
#[macro_use]
extern crate structopt;

use std::io;
use std::collections::HashSet;
use std::io::prelude::*;
use diesel::prelude::*;
use structopt::StructOpt;
use chrono::prelude::*;
use env_logger::{LogBuilder, LogTarget};
use rrinlog_core::parser;
use rrinlog_core::models::NewLog;

mod options;

fn main() {
    init_logging().expect("Logging to initialize");

    let opt = options::Opt::from_args();
    let ips: HashSet<String> = opt.filter_ips.into_iter().collect();
    if opt.dry_run {
        dry_run();
    } else {
        persist_logs(opt.buffer, &opt.db, &ips);
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

fn persist_logs(threshold: usize, db: &str, ips: &HashSet<String>) {
    let conn = SqliteConnection::establish(db)
        .unwrap_or_else(|_| panic!("Error connecting to {}", db));

    // To avoid allocating a string for each line read from stdin and to buffer data so that we
    // batch insert into the db, we keep around the same `n` strings for the whole duration of the
    // application. Since these strings are kept around forever, they will grow to the maximum url
    // size allowed by nginx, which defaults to 1k. So if the buffer size is 10, these strings will
    // contribute a max of 10k to mem usage.
    let mut buffer: Vec<String> = vec![String::new(); threshold];
    let mut buf_ind = 0;
    let stdin = io::stdin();
    let mut locked_stdin = stdin.lock();
    while locked_stdin.read_line(&mut buffer[buf_ind]).unwrap_or(0) > 0 {
        buf_ind += 1;
        if buf_ind >= threshold {
            insert_buffer(&conn, &buffer, ips);
            buf_ind = 0;

            // Remove the parsed lines, but keep the allocated space for them
            buffer.iter_mut().for_each(String::clear);
        }
    }

    // Flush anything else that exists in the buffer
    if buf_ind > 0 {
        insert_buffer(&conn, &buffer[..buf_ind], ips)
    }
}

fn dry_run() {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let stdin = io::stdin();
    let mut line = String::new();
    let mut locked_stdin = stdin.lock();
    while locked_stdin.read_line(&mut line).unwrap_or(0) > 0 {
        match parser::parse_nginx_line(line.trim()) {
            // Both Ok and Err branches halt writing if the line can't be ouput.
            // For instance, this occurs when rrinlog output is piped to head
            Ok(log) => if writeln!(&mut handle, "line: {}", log).is_err() {
                break;
            },
            Err(ref e) => if writeln!(&mut handle, "error: {}", e).is_err() {
                break;
            },
        }

        line.clear();
    }
}

/// If SQLite transaction successfully acquired, `insert_buffer` will drain the provided buffer of
/// log lines even if the line can't be parsed or inserted.
fn insert_buffer<T: AsRef<str>>(conn: &SqliteConnection, buffer: &[T], ips: &HashSet<String>) {
    use rrinlog_core::schema::logs;

    let start = Utc::now();
    let init_len = buffer.len();

    let lines: Vec<NewLog> = buffer
        .iter()
        .map(|line| line.as_ref().trim())
        .map(|line| parser::parse_nginx_line(line))
        .inspect(|line| {
            // If we can't parse a line, yeah that sucks but it's bound to happen so discard
            // the line after it's logged for the attentive sysadmin
            if let Err(ref e) = *line {
                error!("Parsing error: {}", e);
            }
        })
        .filter_map(Result::ok)

        // Filter out black listed ips
        .filter(|x| x.remote_addr.map(|s| !ips.contains(s)).unwrap_or(true))
        .collect();

    // Now that we have all the successfully parsed logs, insert them into the db. If no lines need
    // to be inserted, skip needlessly locking the db
    if !lines.empty() {
        let db_res = diesel::insert_into(logs::table)
            .values(&lines)
            .execute(conn);

        // If inserting into the db fails, log the error, but still discard the messages, so we
        // remain light on memory usage. Never panic as we're supposed to be a long lived
        // application
        if let Err(ref e) = db_res {
            error!("Insertion error: {}", e);
            return;
        }
    }

    let end = Utc::now();
    let dur = end.signed_duration_since(start);
    info!(
        "Parsing and inserting {} out of {} records took {}us",
        lines.len(),
        init_len,
        dur.num_microseconds().unwrap()
    );
}

#[cfg(test)]
mod tests {
    extern crate assert_cli;
    extern crate environment;
    extern crate tempdir;

    use std::path::PathBuf;
    use std::env;

    #[test]
    fn test_dry_run_empty_input() {
        assert_cli::Assert::main_binary()
            .with_args(&["--dry-run"])
            .succeeds()
            .unwrap();
    }

    #[test]
    fn test_dry_run_with_input() {
        let fail_line = "Cats are alright";
        let success_line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        assert_cli::Assert::main_binary()
            .with_args(&["--dry-run"])
            .stdin(format!("{}\n{}", fail_line, success_line))
            .succeeds()
            .stdout()
            .contains("line: ")
            .stdout()
            .contains("error: ")
            .unwrap();
    }

    #[test]
    fn run_db_test() {
        let tmp_dir = tempdir::TempDir::new("rrinlog").unwrap();
        let tmp_path = tmp_dir.path().join("logs.db");
        let tmp = tmp_path.to_str().unwrap();
        let migration_dir = PathBuf::from(r"../migrations");
        let migration = migration_dir.to_str().unwrap();
        println!("Current dir: {:?}", env::current_dir());
        assert_cli::Assert::command(&["diesel"])
            .with_args(&["setup", "--migration-dir", migration, "--database-url", tmp])
            .succeeds()
            .unwrap();

        let fail_line = "Cats are alright";
        let success_line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        let skip_line =
            r#"127.0.0.2 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        assert_cli::Assert::main_binary()
            .with_env(environment::Environment::inherit().insert("RUST_LOG", "INFO"))
            .with_args(&["--buffer", "1", "--filter-ip", "127.0.0.2", "--filter-ip", "127.0.0.3", "--db", tmp])
            .stdin(format!("{}\n{}\n{}", fail_line, success_line, skip_line))
            .succeeds()
            .stdout().satisfies(|out| out.lines().count() == 4, "4 lines")
            .stdout().contains("Text did not match regex `Cats are alright`")
            .stdout().satisfies(|out| {
                let lines: Vec<&str> = out.lines().skip(1).collect();
                lines.len() == 3 &&
                lines[0].contains("inserting 0 out of 1 records") &&
                lines[1].contains("inserting 1 out of 1 records") &&
                lines[2].contains("inserting 0 out of 1 records")
            }, "correct lines")
            .unwrap();
    }
}
