use clap::{Parser, Subcommand};
use rperf3::{server::Server, Config, Protocol};
use std::time::Duration;

/// Parse bandwidth string with K/M/G suffix (in bits per second)
/// Examples: "100M" = 100 Mbps, "1G" = 1 Gbps, "500K" = 500 Kbps
fn parse_bandwidth(s: &str) -> anyhow::Result<u64> {
    let s = s.trim();

    if s.is_empty() {
        anyhow::bail!("Bandwidth cannot be empty");
    }

    let (number_str, multiplier) = if s.ends_with('G') || s.ends_with('g') {
        (&s[..s.len() - 1], 1_000_000_000u64)
    } else if s.ends_with('M') || s.ends_with('m') {
        (&s[..s.len() - 1], 1_000_000u64)
    } else if s.ends_with('K') || s.ends_with('k') {
        (&s[..s.len() - 1], 1_000u64)
    } else {
        (s, 1u64)
    };

    let number: u64 = number_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid bandwidth number: {}", number_str))?;

    Ok(number * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bandwidth_kilobits() {
        assert_eq!(parse_bandwidth("500K").unwrap(), 500_000);
        assert_eq!(parse_bandwidth("500k").unwrap(), 500_000);
    }

    #[test]
    fn test_parse_bandwidth_megabits() {
        assert_eq!(parse_bandwidth("100M").unwrap(), 100_000_000);
        assert_eq!(parse_bandwidth("100m").unwrap(), 100_000_000);
    }

    #[test]
    fn test_parse_bandwidth_gigabits() {
        assert_eq!(parse_bandwidth("1G").unwrap(), 1_000_000_000);
        assert_eq!(parse_bandwidth("1g").unwrap(), 1_000_000_000);
    }

    #[test]
    fn test_parse_bandwidth_plain_number() {
        assert_eq!(parse_bandwidth("1000000").unwrap(), 1_000_000);
    }

    #[test]
    fn test_parse_bandwidth_with_whitespace() {
        assert_eq!(parse_bandwidth(" 100M ").unwrap(), 100_000_000);
    }

    #[test]
    fn test_parse_bandwidth_invalid() {
        assert!(parse_bandwidth("").is_err());
        assert!(parse_bandwidth("abc").is_err());
        assert!(parse_bandwidth("M").is_err());
    }
}

#[derive(Parser)]
#[command(name = "rperf3")]
#[command(author, version, about = "Network performance measurement tool", long_about = None)]
#[command(after_help = "EXAMPLES:
    Start server:
        rperf3 server
        rperf3 server --port 5201 --bind 192.168.1.100

    Run TCP test:
        rperf3 client 192.168.1.100
        rperf3 client 192.168.1.100 --time 30 --interval 2

    Run UDP test with bandwidth limit:
        rperf3 client 192.168.1.100 --udp --bandwidth 100M
        rperf3 client 192.168.1.100 -u -b 1G -t 60

    Reverse mode (server sends):
        rperf3 client 192.168.1.100 --reverse
        rperf3 client 192.168.1.100 -R -b 500M

BANDWIDTH NOTATION:
    K = Kilobits (1,000 bits/sec)     Example: 500K = 500 Kbps
    M = Megabits (1,000,000 bits/sec) Example: 100M = 100 Mbps
    G = Gigabits (1,000,000,000 bits) Example: 1G = 1 Gbps")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start server mode and listen for client connections
    #[command(visible_alias = "s")]
    Server {
        /// Port number to listen on [default: 5201]
        #[arg(short, long, default_value = "5201")]
        port: u16,

        /// Bind to a specific IP address (default: 0.0.0.0, all interfaces)
        #[arg(short, long, value_name = "ADDRESS")]
        bind: Option<String>,

        /// Use UDP protocol instead of TCP
        #[arg(short, long)]
        udp: bool,
    },

    /// Start client mode and connect to a server
    #[command(visible_alias = "c")]
    Client {
        /// Server hostname or IP address to connect to
        #[arg(value_name = "SERVER")]
        server: String,

        /// Server port number to connect to [default: 5201]
        #[arg(short, long, default_value = "5201")]
        port: u16,

        /// Use UDP protocol instead of TCP
        #[arg(short, long)]
        udp: bool,

        /// Duration of test in seconds [default: 10]
        #[arg(short = 't', long, value_name = "SECONDS", default_value = "10")]
        time: u64,

        /// Target bandwidth for UDP tests (e.g., 100M, 1G, 500K)
        /// Applies to UDP and TCP reverse mode. Use K/M/G suffix for units.
        #[arg(short, long, value_name = "BANDWIDTH")]
        bandwidth: Option<String>,

        /// Buffer/packet size in bytes [default: 128K for TCP, 1500 for UDP]
        #[arg(short = 'l', long, value_name = "BYTES", default_value = "131072")]
        length: usize,

        /// Number of parallel streams to use [default: 1]
        #[arg(short = 'P', long, value_name = "NUM", default_value = "1")]
        parallel: usize,

        /// Run in reverse mode (server sends, client receives)
        #[arg(short = 'R', long)]
        reverse: bool,

        /// Output results in JSON format for machine parsing
        #[arg(short = 'J', long)]
        json: bool,

        /// Interval between periodic reports in seconds [default: 1]
        #[arg(short, long, value_name = "SECONDS", default_value = "1")]
        interval: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Disable logs in release builds, enable info level in debug builds
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    #[cfg(not(debug_assertions))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Server { port, bind, udp } => {
            let protocol = if udp { Protocol::Udp } else { Protocol::Tcp };

            let mut config = Config::server(port).with_protocol(protocol);

            if let Some(bind_addr) = bind {
                config.bind_addr = Some(bind_addr.parse()?);
            }

            let server = Server::new(config);
            server.run().await?;
        }

        Commands::Client {
            server,
            port,
            udp,
            time,
            bandwidth,
            length,
            parallel,
            reverse,
            json,
            interval,
        } => {
            let protocol = if udp { Protocol::Udp } else { Protocol::Tcp };

            // Use 1500 bytes for UDP if default length was specified
            let buffer_size = if udp && length == 131072 {
                1500
            } else {
                length
            };

            let mut config = Config::client(server, port)
                .with_protocol(protocol)
                .with_duration(Duration::from_secs(time))
                .with_buffer_size(buffer_size)
                .with_parallel(parallel)
                .with_reverse(reverse)
                .with_json(json)
                .with_interval(Duration::from_secs(interval));

            if let Some(bw_str) = bandwidth {
                let bw = parse_bandwidth(&bw_str)?;
                config = config.with_bandwidth(bw);
            }

            use rperf3::client::Client;

            let client = Client::new(config)?;
            client.run().await?;
        }
    }

    Ok(())
}
