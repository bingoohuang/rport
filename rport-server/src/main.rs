use clap::Parser;
use rport_server::create_router;
use std::net::SocketAddr;

mod cli;
use cli::ServerCli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = ServerCli::parse();

    // Initialize tracing - only show logs from rport modules
    tracing_subscriber::fmt().init();

    let app = create_router();

    let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
    println!("Server running on http://{}", cli.addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
