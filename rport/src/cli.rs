use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rport")]
#[command(about = "Remote port forwarding client and agent")]
pub struct Cli {
    /// Configuration file path
    #[arg(short = 'f', long = "conf")]
    pub config: Option<PathBuf>,
    /// Server URL
    #[arg(short, long, default_value = "http://127.0.0.1:3000")]
    pub server: String,
    /// Authentication token
    #[arg(short = 'k', long)]
    pub token: Option<String>,
    /// Agent ID (required for ProxyCommand and port forwarding modes)
    #[arg(short, long)]
    pub id: Option<String>,
    /// List available agents
    #[arg(short, long)]
    pub list: bool,
    /// Target address for agent mode (e.g., 127.0.0.1:22 or just 22)
    #[arg(short = 't', long)]
    pub target: Option<String>,
    /// Local port for CLI port forwarding mode
    #[arg(short, long)]
    pub port: Option<u16>,
    /// Run as daemon (detach from terminal)
    #[arg(short = 'd', long)]
    pub daemon: bool,
    /// Log file path for daemon mode
    #[arg(long = "log-file")]
    pub log_file: Option<PathBuf>,
    /// ProxyCommand arguments: hostname and port (for SSH ProxyCommand usage)
    #[arg(value_name = "HOST")]
    pub proxy_args: Vec<String>,
}
