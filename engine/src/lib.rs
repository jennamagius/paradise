pub mod config;
mod paradise;

pub async fn run() -> anyhow::Result<()> {
    let config = <crate::config::Config as ::structopt::StructOpt>::from_args();
    let paradise = crate::paradise::Paradise::new(config).await?;
    paradise.run().await
}
