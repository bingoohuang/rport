use anyhow::Result;
use clap::Parser;
use rport_server::{handler::create_router, TurnServer};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
mod cli;
use cli::ServerCli;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = ServerCli::parse();

    tracing_subscriber::fmt().init();
    // Start TURN server
    let addr_parts: Vec<&str> = cli.addr.split(':').collect();
    let ip = if let Ok(parsed_ip) = addr_parts.get(0).unwrap().parse::<IpAddr>() {
        parsed_ip
    } else {
        get_first_non_loopback_interface()?
    };

    let public_ip = cli.public_ip.clone();
    let turn_addr = cli.turn_addr.unwrap_or_else(|| format!("{}:13479", ip));
    let turn_server = Arc::new(TurnServer::new(cli.disable_turn, &turn_addr, public_ip).await?);
    turn_server.start().await.ok();

    // Start TURN server in background
    let turn_server_clone = turn_server.clone();
    let app = create_router(turn_server);
    let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
    println!("Server running on http://{}", cli.addr);
    tokio::select! {
        _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
            ) => {}
        _ = turn_server_clone.cleanup_expired_credentials() =>{}
    };
    turn_server_clone.close().await.ok();
    Ok(())
}
