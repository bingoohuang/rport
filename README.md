# RPort - WebRTC-based remote port forwarding tool written in Rust

RPort is a modern, WebRTC-based remote port forwarding tool written in Rust. It enables secure peer-to-peer connections for port forwarding, remote access, and network tunneling without requiring complex NAT traversal configurations.

## Features

- 🚀 **WebRTC-based P2P connections** - Direct peer-to-peer tunneling
- 🔒 **Secure tunneling** - End-to-end encrypted connections
- 📁 **Configuration file support** - TOML-based configuration with CLI override
- 🌐 **IPv6 filtering** - Automatic IPv6 candidate filtering for better compatibility
- 🔧 **Multiple operation modes** - Agent, client, and proxy modes
- 🔄 **Background daemon support** - Run as a system daemon with custom log files
- 📊 **Structured logging** - Comprehensive logging with tracing support
- ⚡ **High performance** - Built with Tokio async runtime

## Architecture

```
┌─────────────┐    WebRTC P2P    ┌─────────────┐
│   Client    │◄────────────────►│    Agent    │
│             │                  │             │
│ rport -p    │                  │ rport -t    │
│ 8080:22     │                  │ 22 --id     │
└─────────────┘                  └─────────────┘
       │                                │
       │                                │
       ▼                                ▼
┌─────────────┐                ┌─────────────┐
│Local Service│                │Remote Server│
│ :8080       │                │ :22 (SSH)   │
└─────────────┘                └─────────────┘
```

## Quick Start

### 1. Build the Project

```bash
# Clone the repository
git clone <repository-url>
cd rport

# Build all components
cargo build --release
```

### 2. Start the Server

```bash
# Start the coordination server
./target/release/rport-server --addr 0.0.0.0:3000
```

### 3. Configure and Run Agent

Create a configuration file:

```bash
# Copy example config
cp example-config.toml ~/.rport.toml

# Edit with your settings
nano ~/.rport.toml
```

Start the agent on the remote machine:

```bash
# Run as daemon with custom log file
./target/release/rport --target 22 --id my-server --daemon --log-file /var/log/rport-agent.log

# Or run in foreground for testing
./target/release/rport --target 22 --id my-server --token your-auth-token
```

### 4. Connect from Client

```bash
# Forward local port 8080 to remote SSH (port 22)
./target/release/rport --port 8080 --id my-server --token your-auth-token

# Now you can SSH through the tunnel
ssh user@localhost -p 8080
```

## Configuration

### Configuration File

RPort supports configuration files to avoid repeatedly specifying command-line arguments. The configuration file uses TOML format and is loaded in the following order:

1. **Custom path**: Specified with `--conf/-f` parameter
2. **User home**: `~/.rport.toml` (automatically detected)
3. **CLI override**: Command-line arguments override file settings

#### Default Configuration Location

Place your configuration at `~/.rport.toml`:

```toml
# Authentication token (required)
# This token must match between client, agent, and server
token = "your-secure-token-here"

# ICE servers for WebRTC NAT traversal
# Multiple STUN servers for redundancy
[[ice_servers]]
urls = ["stun:stun.l.google.com:19302"]

[[ice_servers]]
urls = ["stun:restsend.com:3478"]

# TURN server example (for restrictive networks)
# Uncomment and configure for corporate networks
# [[ice_servers]]
# urls = ["turn:your-turn-server.com:3478"]
# username = "your-username"
# password = "your-password"

# Advanced ICE server configuration with authentication
# [[ice_servers]]
# urls = ["turns:secure-turn.example.com:5349"]
# username = "user123"
# password = "pass456"
# credential_type = "password"
```

#### Configuration File Management

```bash
# Create configuration from example
cp example-config.toml ~/.rport.toml

# Edit configuration
nano ~/.rport.toml

# Use custom configuration file
rport --conf /path/to/custom.toml --target 22 --id server1

# Override token from command line (useful for CI/CD)
rport --conf ~/.rport.toml --token $SECRET_TOKEN --target 22 --id server1

# Validate configuration
rport --conf ~/.rport.toml --help
```

#### Configuration Security

```bash
# Set proper permissions for configuration file
chmod 600 ~/.rport.toml

# Store sensitive tokens in environment variables
export RPORT_TOKEN="your-secret-token"
rport --token $RPORT_TOKEN --target 22 --id server1
```

### Command Line Options

#### Agent Mode (Remote Machine)

```bash
rport --target <PORT> --id <AGENT_ID> [OPTIONS]

Options:
  -t, --target <PORT>           Port to forward connections to
  -i, --id <AGENT_ID>          Unique agent identifier
  -s, --server <URL>           Server URL [default: http://127.0.0.1:3000]
  --token <TOKEN>              Authentication token
  -f, --config <FILE>          Configuration file path
  -d, --daemon                 Run as daemon
  --log-file <FILE>            Custom log file for daemon mode
```

#### Client Mode (Local Machine)

```bash
rport --port <LOCAL> --id <AGENT_ID> [OPTIONS]

Options:
  -p, --port <LOCAL>           Port mapping (local)
  -i, --id <AGENT_ID>          Target agent identifier
  -s, --server <URL>           Server URL [default: http://127.0.0.1:3000]
  --token <TOKEN>              Authentication token
  -f, --config <FILE>          Configuration file path
```

#### Server Mode

```bash
rport-server [OPTIONS]

Options:
  -a, --addr <ADDRESS>         Server bind address [default: 127.0.0.1:3000]
```

## Usage Examples

### SSH Tunneling

1. **Setup agent on remote server:**
   ```bash
   # Run SSH agent on remote machine
   ./rport --target 22 --id production-server --daemon
   ```

2. **Connect from local machine:**
   ```bash
   # Forward local port 2222 to remote SSH
   ./rport --port 2222 --id production-server
   
   # SSH through tunnel
   ssh user@localhost -p 2222
   ```

### Web Service Access

1. **Agent on server with web service:**
   ```bash
   ./rport --target 80 --id web-server --daemon
   ```

2. **Access from local browser:**
   ```bash
   ./rport --port 8080 --id web-server
   # Visit http://localhost:8080
   ```

### Database Tunneling

```bash
# Agent on database server
./rport --target 5432 --id db-server --daemon

# Client connection
./rport --port 5432 --id db-server
# Connect to localhost:5432 with your database client
```

### List Available Agents

```bash
./rport --list --token your-auth-token
```

## Advanced SSH Integration

### SSH ProxyCommand Usage

RPort can be integrated with SSH using ProxyCommand for seamless remote access without manual port forwarding.

#### Method 1: Direct ProxyCommand

```bash
# Single SSH connection through RPort tunnel
ssh -o ProxyCommand='rport --id production-server --token your-token' user@localhost

# With custom server
ssh -o ProxyCommand='rport --server http://vpn.company.com:3000 --id web-server --token your-token' user@localhost
```

#### Method 2: SSH Config Integration

Create or edit `~/.ssh/config`:

```ssh-config
# RPort tunnel configuration
Host production-server
    HostName localhost
    User your-username
    Port 2222
    ProxyCommand rport --conf ~/.rport.toml --id production-server
    ServerAliveInterval 30
    ServerAliveCountMax 3

# Multiple servers through RPort
Host web-server
    HostName localhost
    User ubuntu
    Port 3333
    ProxyCommand rport --conf ~/.rport.toml --id web-server

Host db-server
    HostName localhost
    User postgres
    Port 5432
    ProxyCommand rport --conf ~/.rport.toml --id database-server
    LocalForward 5432 localhost:5432

# Jump host configuration
Host jump-server
    HostName localhost
    User admin
    Port 2222
    ProxyCommand rport --conf ~/.rport.toml --id jump-host

Host internal-server
    HostName 192.168.1.100
    User developer
    ProxyJump jump-server
```
## Daemon Mode

RPort supports running as a background daemon with comprehensive logging:

```bash
# Start daemon with default log location
./rport --target 22 --id server1 --daemon

# Start daemon with custom log file
./rport --target 22 --id server1 --daemon --log-file /var/log/rport/agent.log

# Check daemon status
ps aux | grep rport

# View logs
tail -f /var/log/rport/agent.log
```

## Troubleshooting

### Common Issues

1. **Connection fails through NAT:**
   - Add TURN servers to your configuration
   - Check firewall settings
   - Verify ICE server connectivity

2. **Token authentication errors:**
   - Ensure token matches between client and agent
   - Check configuration file syntax
   - Verify token in server logs

3. **Port binding errors:**
   - Check if local port is already in use
   - Run with different port numbers
   - Verify permissions for privileged ports (<1024)

4. **Daemon not starting:**
   - Check log file permissions
   - Verify configuration file path
   - Ensure target service is accessible

### Debug Mode

Enable debug logging for detailed troubleshooting:

```bash
RUST_LOG=rport=debug ./rport --target 22 --id debug-agent
```

### Log Locations

- **Daemon mode default logs:** `/tmp/rport-*.log`
- **Custom daemon logs:** Specified by `--log-file`
- **Foreground mode:** Standard output/error

## Security Considerations

- Use strong authentication tokens
- Configure TURN servers with authentication for production
- Monitor log files for suspicious activity
- Run agents with minimal required privileges
- Use configuration files with appropriate file permissions (600)

## Support

For issues and questions:
- Check the troubleshooting section
- Review log files for error details
- Open an issue with reproduction steps
