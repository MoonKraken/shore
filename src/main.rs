mod app;
mod database;
mod markdown;
pub mod model;
mod model_select_modal;
pub mod provider;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use database::Database;
use std::sync::Arc;
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, help = "Database name (without .db extension)")]
    database: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // only log if the SHORE_LOG env var is set
    // when doing this the user needs to make sure to pipe stderr
    // to a file and tail the file if they want to follow the logs
    // otherwise the TUI interface will be ruined by log output
    if let Ok(_) = std::env::var("SHORE_LOG") {
        tracing_subscriber::fmt()
            .with_span_events(FmtSpan::ENTER | FmtSpan::CLOSE)
            .with_writer(std::io::stderr)
            .init();
    }

    let cli = Cli::parse();
    let db_name = cli.database.unwrap_or_else(|| "default".to_string());

    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let shore_dir = home_dir.join(".shore");
    std::fs::create_dir_all(&shore_dir)?;

    let db_path = shore_dir.join(format!("{}.db", db_name));
    let database = Database::new(db_path).await?;

    let (mut app, user_event_rx) = App::new(database).await?;

    // Spawn background task to refresh models from provider APIs
    {
        // this is a little ugly but I decided its preferable to using locks everywhere
        // since it only happens once on startup. The locks approach would continue to affect
        // performance long after we no longer need them.
        let database = Arc::clone(&app.database);
        let providers = app.providers.clone();
        let provider_clients = app.provider_clients.clone();
        let all_models = app.all_models.clone();
        let event_tx = app.user_event_tx.clone();

        tokio::spawn(async move {
            App::refresh_models_with_provider_api(
                database,
                providers,
                provider_clients,
                all_models,
                event_tx,
            )
            .await;
        });
    }

    app.run(user_event_rx).await?;

    Ok(())
}
