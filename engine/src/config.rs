#[derive(::structopt::StructOpt)]
pub struct Config {
    /// An sqlx connection string indicating how to connect to the database
    #[structopt(long, env = "PARADISE_DB", hide_env_values = true)]
    pub db: String,
}
