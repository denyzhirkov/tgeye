use std::path::Path;

use tgeye_config::AppConfig;

use super::env;

pub async fn run(data_dir: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        data_dir.is_dir(),
        "data directory {} does not exist; run `tgeye init` first",
        data_dir.display()
    );
    let config = AppConfig::load(data_dir, env)?;
    let db_path = config.database_path(data_dir);
    let pool = tgeye_storage::connect(&db_path).await?;
    let pending = tgeye_storage::pending_migrations(&pool).await?;
    tgeye_storage::run_migrations(&pool).await?;
    println!("Applied {pending} migration(s); database is up to date.");
    Ok(())
}
