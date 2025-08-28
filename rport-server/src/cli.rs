use clap::Parser;

#[derive(Parser)]
#[command(name = "rport-server")]
#[command(about = "Remote port forwarding server")]
pub struct ServerCli {
    /// Server bind address
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    pub addr: String,
}
