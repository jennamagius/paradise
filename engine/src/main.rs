#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ::env_logger::init();
    ::paradise::run().await
}
