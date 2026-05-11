#[tokio::main]
async fn main() -> anyhow::Result<()> {
    hsb_mock_endpoints::run_default_endpoint(4).await
}
