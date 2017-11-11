#[derive(StructOpt, Debug)]
#[structopt(name = "rrinlog-server", about = "Simple JSON datasource endpointf or grafana")]
pub struct Opt {
    #[structopt(long = "db", help = "Filepath to sqlite database", default_value = "logs.db")]
    pub db: String,

    #[structopt(long = "ip", help = "Local IP address to ignore from logs")] pub ip: String,
}
