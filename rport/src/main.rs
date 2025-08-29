use clap::Parser;
mod agent;
mod cli;
mod client;
mod config;
#[cfg(unix)]
mod daemon;
mod sdp_utils;
mod webrtc_config;

use agent::Agent;
use cli::Cli;
use client::CliClient;
use config::RportConfig;
use webrtc_config::WebRTCConfig;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle daemon mode before doing anything else, including tokio runtime
    if cli.daemon {
        #[cfg(unix)]
        {
            // Use specified log file or create default path
            let log_file = if let Some(log_path) = &cli.log_file {
                log_path.to_string_lossy().to_string()
            } else {
                // Default log file paths in /tmp
                if cli.target.is_some() {
                    "/tmp/rport-agent.log".to_string()
                } else if cli.port.is_some() {
                    "/tmp/rport-forward.log".to_string()
                } else if !cli.proxy_args.is_empty() {
                    "/tmp/rport-proxy.log".to_string()
                } else {
                    "/tmp/rport.log".to_string()
                }
            };
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

async fn async_main(cli: Cli) -> anyhow::Result<()> {
    // Load configuration
    let mut config = if let Some(config_path) = &cli.config {
        RportConfig::load_from_file(config_path)?
    } else {
        RportConfig::load_default()?
    };

    // Merge CLI token with config
    config.merge_with_cli(cli.token.clone(), cli.server.clone(), cli.id.clone());

    // Get the final token
    let token = config.token.clone().ok_or_else(|| {
        anyhow::anyhow!("Token is required. Provide it via --token or in config file")
    })?;
    let server = config.server.clone().ok_or_else(|| {
        anyhow::anyhow!("Server is required. Provide it via --server or in config file")
    })?;

    // Create WebRTC configuration
    let webrtc_config = WebRTCConfig::new(config.get_ice_servers());

    // Initialize tracing
    // In daemon mode, logs will be written to the log file
    use tracing_subscriber::{self, filter::EnvFilter};
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("rport=info,turn=warn,webrtc=warn"))
        .init();

    // Determine mode based on arguments
    if cli.list {
        let client = CliClient::new(server, token, webrtc_config);
        let agents = client.list_agents().await?;
        println!("Available agents:");
        for agent in agents {
            println!("  - {}", agent);
        }
    } else if let Some(target) = cli.target {
        // Agent mode
        let (host, port) = parse_target(&target)?;
        let agent_id = cli
            .id
            .unwrap_or_else(|| format!("agent-{}", std::process::id()));
        let agent = Agent::new(server, token, agent_id, host, port, webrtc_config);
        agent.run().await?;
    } else if let Some(local_port) = cli.port {
        // CLI port forwarding mode
        let agent_id = cli.id.ok_or_else(|| {
            anyhow::anyhow!("Agent ID is required for port forwarding mode. Use --id <AGENT_ID>")
        })?;
        let client = CliClient::new(server, token, webrtc_config);
        client.connect_port_forward(agent_id, local_port).await?;
    } else {
        let agent_id = cli.id.ok_or_else(|| {
            anyhow::anyhow!("Agent ID is required for port forwarding mode. Use --id <AGENT_ID>")
        })?;
        let client = CliClient::new(server, token, webrtc_config);
        client.connect_proxy_command(agent_id).await?;
    }

    Ok(())
}

fn parse_target(target: &str) -> anyhow::Result<(String, u16)> {
    if let Some(colon_pos) = target.rfind(':') {
        let host = target[..colon_pos].to_string();
        let port: u16 = target[colon_pos + 1..].parse()?;
        Ok((host, port))
    } else {
        // Just a port number
        let port: u16 = target.parse()?;
        Ok(("127.0.0.1".to_string(), port))
    }
}
