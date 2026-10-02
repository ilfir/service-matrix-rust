use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
    time::Duration,
};

use service_matrix_rust::{build_app, config::Config, storage::DictionaryStore};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::args().nth(1).as_deref() == Some("--healthcheck") {
        run_healthcheck()?;
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env().map_err(std::io::Error::other)?;
    let address = config.socket_addr();
    let store = Arc::new(DictionaryStore::load(
        config.data_dir.clone(),
        config.resources_dir.clone(),
    )?);
    let app = build_app(config, store);
    let listener = TcpListener::bind(address).await?;
    info!(%address, "service-matrix-rust listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn run_healthcheck() -> std::io::Result<()> {
    let port = env::var("SERVICE_MATRIX_PORT").unwrap_or_else(|_| "8080".to_owned());
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if response.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(std::io::Error::other("health endpoint did not return 200"))
    }
}

async fn shutdown_signal() {
    let control_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install termination handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = control_c => {},
        () = terminate => {},
    }
}
