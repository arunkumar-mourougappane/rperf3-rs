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
#[command(about = "A Rust implementation of iperf3 - network performance testing tool", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in server mode
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "5201")]
        port: u16,

        /// Bind to specific address
        #[arg(short, long)]
        bind: Option<String>,

        /// Use UDP instead of TCP
        #[arg(short, long)]
        udp: bool,
    },

    /// Run in client mode
    Client {
        /// Server address to connect to
        server: String,

        /// Port to connect to
        #[arg(short, long, default_value = "5201")]
        port: u16,

        /// Use UDP instead of TCP
        #[arg(short, long)]
        udp: bool,

        /// Test duration in seconds
        #[arg(short = 't', long, default_value = "10")]
        time: u64,

        /// Target bandwidth (for UDP). Supports K/M/G suffix (e.g., 100M, 1G, 500K)
        #[arg(short, long)]
        bandwidth: Option<String>,

        /// Buffer size in bytes
        #[arg(short = 'l', long, default_value = "131072")]
        length: usize,

        /// Number of parallel streams
        #[arg(short = 'P', long, default_value = "1")]
        parallel: usize,

        /// Run in reverse mode (server sends, client receives)
        #[arg(short = 'R', long)]
        reverse: bool,

        /// Output in JSON format
        #[arg(short = 'J', long)]
        json: bool,

        /// Interval for periodic reports in seconds
        #[arg(short, long, default_value = "1")]
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
