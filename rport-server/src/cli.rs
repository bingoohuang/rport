use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rport-server")]
#[command(about = "Remote port forwarding server")]
pub struct ServerCli {
    /// Server bind address
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    pub addr: String,

    /// Disable TURN server
    #[arg(long, default_value_t = false)]
    pub disable_turn: bool,

    #[arg(short, long, default_value = "0.0.0.0:13478")]
    pub turn_addr: Option<String>,

    /// Public IP address for TURN server
    #[arg(long)]
    pub public_ip: Option<String>,

    /// Run as daemon (detach from terminal)
    #[arg(short = 'd', long)]
    pub daemon: bool,

    /// Log file path for daemon mode
    #[arg(long = "log-file")]
    pub log_file: Option<PathBuf>,
}
