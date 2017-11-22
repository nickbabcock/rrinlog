# rrinlog

rrinlog is my attempt at [Replacing Elasticsearch with Rust and SQLite](https://nbsoftsolutions.com/blog/replacing-elasticsearch-with-rust-and-sqlite) for my nginx access logs, as Elasticsearch is a resource hungry application even at idle. rrinlog's success has been outstanding, with a 100x reduction in memory, 1000x reduction in CPU usage, and 100x reduction in disk usage.

This project contains two binaries:

- `rrinlog` is for consuming nginx acces logs and storing them in a SQLite database. This binary may be built on Rust stable.
- `rrinlog-server` exposes this SQLite database according to Grafana's [JSON API datasource](https://github.com/grafana/simple-json-datasource). This binary requires Rust nightly.

This project currently isn't meant at replacing Elasticsearch for the general populous for the following reasons:

### Custon Nginx Access Log

`rrinlog` ingests a custom nginx access log format:

```
log_format vhost    '$remote_addr - $remote_user [$time_local] '
                    '"$request" $status $body_bytes_sent '
                    '"$http_referer" "$http_user_agent" "$host"';
```

Any other format would likely result in parsing errors.

### Hardcoded SQL Queries

`rrinlog-server` let's me know what my top blog articles with the following SQL query:

```sql
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
```

This SQL query is tailored to me and how my blog is setup, so make no mistake that the intended audience with this query is soley me :smile:

### Limited Endpoints

These hardcoded SQL queries are needed as Grafana doesn't support SQLite as a native datasource. One day it may be supported like Mysql and Postgres, but until that day, `rrinlog-server` contains only a limited set of visualizations:

- What are my top blog articles
- How much outbound web data is leaving the server to other external IPs
- How many requests are being serviced by other virtual hosts

### No GeoIP Capabilities

Elasticsearch has the ability to take an IP address and turn it into a
location. This is called
[GeoIP](https://www.elastic.co/blog/geoip-in-the-elastic-stack). I had a
Grafana panel showing the top visiting cities, which is novel but not critical
to monitor. Migrating from Elasticsearch meant I had to remove the
visualization.
