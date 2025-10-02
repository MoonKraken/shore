mod app;
mod database;
mod ui;
mod handlers;
mod markdown;
pub mod model;
pub mod provider;

use anyhow::Result;
use app::App;
use clap::Parser;
use database::Database;
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, help = "Database name (without .db extension)")]
    database: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::ENTER | FmtSpan::CLOSE)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let db_name = cli.database.unwrap_or_else(|| "default".to_string());

    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let shore_dir = home_dir.join(".shore");
    std::fs::create_dir_all(&shore_dir)?;

    let db_path = shore_dir.join(format!("{}.db", db_name));
    let database = Database::new(db_path).await?;

    let (mut app, user_event_rx) = App::new(database).await?;
    app.run(user_event_rx).await?;

    Ok(())
}
