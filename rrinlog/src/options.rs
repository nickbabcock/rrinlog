#[derive(StructOpt, Debug)]
#[structopt(
    name = "rrinlog",
    about = "Ingests nginx access logs and persists them to SQLite"
)]
pub struct Opt {
    #[structopt(
        short = "d",
        long = "dry-run",
        help = "Print the parsed logs to stdout instead of persisting to the db"
    )]
    pub dry_run: bool,

    #[structopt(long = "filter-ip", help = "Do not store given ip address in the db")]
    pub filter_ips: Vec<String>,

    #[structopt(
        short = "b",
        long = "buffer",
        help = "number of log lines to buffer before inserting into db",
        default_value = "10"
    )]
    pub buffer: usize,

    #[structopt(
        long = "db",
        help = "Filepath to sqlite database",
        default_value = "logs.db"
    )]
    pub db: String,
}
