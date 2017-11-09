#![recursion_limit = "128"]

extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate regex;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate error_chain;

pub mod models;
pub mod parser;
pub mod schema;

#[cfg(all(feature = "unstable", test))]
mod bench {
    extern crate test;
    use parser;

    #[bench]
    fn bench_parse_nginx(b: &mut test::Bencher) {
        let line =
            r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
        b.iter(|| {
            assert!(parser::parse_nginx_line(line).is_ok());
        });
    }
}
