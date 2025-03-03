use tokio;
use anyhow;
use utoo_runtime;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    utoo_runtime::boostrap("").await?;
    Ok(())
}
