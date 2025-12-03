use rperf3::{server::Server, Config, Protocol};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Starting rperf3 server on port 5201...");

    let config = Config::server(5201).with_protocol(Protocol::Tcp);

    let server = Server::new(config);
    server.run().await?;

    Ok(())
}
