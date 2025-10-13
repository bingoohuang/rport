use anyhow::Result;
use clap::Parser;
use cli::ServerCli;
use rport_server::{handler::create_router, TurnServer};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tracing_subscriber::{self, filter::EnvFilter};
mod cli;
#[cfg(unix)]
mod daemon;

pub fn get_first_non_loopback_interface() -> Result<IpAddr> {
    for i in get_if_addrs::get_if_addrs()? {
        if !i.is_loopback() {
            match i.addr {
                get_if_addrs::IfAddr::V4(ref addr) => return Ok(std::net::IpAddr::V4(addr.ip)),
                _ => continue,
            }
        }
    }
    Err(anyhow::anyhow!("No IPV4 interface found"))
}

fn main() -> anyhow::Result<()> {
    let cli = ServerCli::parse();

    // Handle daemon mode before doing anything else, including tokio runtime
    if cli.daemon {
        #[cfg(unix)]
        {
            // Use specified log file or default to /tmp/rport-server.log
            let log_file = cli
                .log_file
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/tmp/rport-server.log".to_string());
            daemon::daemonize_with_log(&log_file)?;
        }
        #[cfg(not(unix))]
        {
            return Err(anyhow::anyhow!(
                "Daemon mode is only supported on Unix systems"
            ));
        }
    }

    // Start tokio runtime after daemon is initialized
    tokio::runtime::Runtime::new()?.block_on(async_main(cli))
}

#[allow(clippy::too_many_arguments)]
async fn async_main(cli: ServerCli) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("rport=info,turn=warn,webrtc=warn"))
        .init();

    // Start TURN server
    let addr_parts: Vec<&str> = cli.addr.split(':').collect();
    let ip: IpAddr = if let Ok(parsed_ip) = addr_parts.get(0).unwrap().parse::<IpAddr>() {
        if parsed_ip.is_unspecified() {
            get_first_non_loopback_interface()?
        } else {
            parsed_ip
        }
    } else {
        get_first_non_loopback_interface()?
    };

    let public_ip = cli.public_ip.clone();
    let turn_addr = cli
        .turn_addr
        .unwrap_or_else(|| format!("{}:13478", ip))
        .parse::<SocketAddr>()?;
    let turn_server = Arc::new(TurnServer::new(cli.disable_turn, turn_addr, public_ip).await?);
    turn_server.start().await.ok();

    // Start TURN server in background
    let turn_server_clone = turn_server.clone();
    let app = create_router(turn_server);
    let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
    println!(
        "Server running on http://{}:{}",
        ip,
        addr_parts.get(1).unwrap_or(&"3000")
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .ok();

    turn_server_clone.close().await.ok();
    Ok(())
}
