#[macro_use]
extern crate criterion;
extern crate rrinlog_core;

use criterion::Criterion;
use rrinlog_core::parser::{parse_date, parse_nginx_line};

fn parse_line_benchmark(c: &mut Criterion) {
    let line =
		r#"127.0.0.1 - - [04/Nov/2017:13:05:35 -0500] "GET /js/embed.min.js HTTP/2.0" 200 20480 "https://nbsoftsolutions.com/blog/monitoring-windows-system-metrics-with-grafana" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/61.0.3163.100 Safari/537.36" "comments.nbsoftsolutions.com""#;
    c.bench_function("parse line", move |b| b.iter(|| parse_nginx_line(line)));
}

fn parse_date_benchmark(c: &mut Criterion) {
    let line = "03/Nov/2017:06:49:45 -0500";
    c.bench_function("parse date", move |b| b.iter(|| parse_date(line)));
}

criterion_group!(benches, parse_line_benchmark, parse_date_benchmark);
criterion_main!(benches);
