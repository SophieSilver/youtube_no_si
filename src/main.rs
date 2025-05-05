use std::time::Duration;

use tracing::{info, instrument, warn};
use tracing_subscriber::EnvFilter;
use youtube_no_si_redux::{run_bot, token::load_token};

const FORCED_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tokio::select! {
        // spawn the bot in a separate task so it does not interfere with the forced shutdown
        _ = tokio::spawn(run_bot(load_token()?)) => {},
        // forcibly shutdown everything after some time after receiving a Ctrl-C
        _ = forced_shutdown() => {}
    }

    Ok(())
}

#[instrument]
async fn forced_shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for the Ctrl-C event");

    info!("^C received, press again for forced shutdown");

    tokio::select! {
        res = tokio::signal::ctrl_c() => {
            res.expect("failed to listen for the Ctrl-C event");
            warn!("forced shutdown initiated, exiting program...");
        }
        _ = tokio::time::sleep(FORCED_SHUTDOWN_TIMEOUT) => {
            warn!("forced shutdown timeout expired, exiting program...");
        }
    };
}
