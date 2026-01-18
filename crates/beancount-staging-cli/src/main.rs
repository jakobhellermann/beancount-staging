use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    beancount_staging_cli::run(std::env::args()).await
}
