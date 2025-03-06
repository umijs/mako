use anyhow;
use tokio;
use utoo_runtime;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    utoo_runtime::bootstrap("").await?;
    Ok(())
}
