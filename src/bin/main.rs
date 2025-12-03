use clap::{Parser, Subcommand};
use rperf3::{server::Server, Config, Protocol};
use std::time::Duration;

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

        /// Target bandwidth in Mbps (for UDP)
        #[arg(short, long)]
        bandwidth: Option<u64>,

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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

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

            let mut config = Config::client(server, port)
                .with_protocol(protocol)
                .with_duration(Duration::from_secs(time))
                .with_buffer_size(length)
                .with_parallel(parallel)
                .with_reverse(reverse)
                .with_json(json)
                .with_interval(Duration::from_secs(interval));

            if let Some(bw) = bandwidth {
                config = config.with_bandwidth(bw * 1_000_000); // Convert Mbps to bps
            }

            use rperf3::client::Client;
            
            let client = Client::new(config)?;
            client.run().await?;
        }
    }

    Ok(())
}
