#[derive(StructOpt, Debug)]
#[structopt(
    name = "rrinlog-server",
    about = "Simple JSON datasource endpointf or grafana"
)]
pub struct Opt {
    #[structopt(
        long = "addr",
        help = "Address to bind to",
        default_value = "127.0.0.1:8000"
    )]
    pub addr: String,

    #[structopt(
        long = "db",
        help = "Filepath to sqlite database",
        default_value = "logs.db"
    )]
    pub db: String,

    #[structopt(long = "ip", help = "Local IP address to ignore from logs")]
    pub ip: String,
}
