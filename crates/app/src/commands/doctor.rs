use std::path::Path;

use tgeye_config::AppConfig;

use super::{env, privacy_hint};

pub async fn run(data_dir: &Path) -> anyhow::Result<()> {
    let mut failures = 0usize;
    let mut check = |name: &str, result: Result<String, String>| match result {
        Ok(detail) => println!("  ✓ {name:<10} {detail}"),
        Err(detail) => {
            failures += 1;
            println!("  ✗ {name:<10} {detail}");
        }
    };

    println!("tgeye doctor");

    if !data_dir.is_dir() {
        check(
            "data dir",
            Err(format!("{} missing — run `tgeye init`", data_dir.display())),
        );
        anyhow::bail!("1 check failed");
    }
    check("data dir", Ok(data_dir.display().to_string()));

    let config = match AppConfig::load(data_dir, env) {
        Ok(config) => {
            check("config", Ok("valid".into()));
            Some(config)
        }
        Err(e) => {
            check("config", Err(e.to_string()));
            None
        }
    };

    check("secrets", secrets_check(data_dir));

    let token = match tgeye_config::load_bot_token(data_dir, env) {
        Ok((token, source)) => {
            check("token", Ok(format!("present ({source})")));
            Some(token)
        }
        Err(e) => {
            check("token", Err(e.to_string()));
            None
        }
    };

    if let Some(token) = token {
        match tgeye_telegram::validate_token(&token).await {
            Ok(me) => check(
                "telegram",
                Ok(format!("@{} — {}", me.username, privacy_hint(&me))),
            ),
            Err(e) => check("telegram", Err(e.to_string())),
        }
    }

    if let Some(config) = config {
        let db_path = config.database_path(data_dir);
        match tgeye_storage::connect(&db_path).await {
            Ok(pool) => match tgeye_storage::pending_migrations(&pool).await {
                Ok(0) => check(
                    "database",
                    Ok(format!("{} — migrations up to date", db_path.display())),
                ),
                Ok(n) => check(
                    "database",
                    Err(format!("{n} pending migrations — run `tgeye migrate`")),
                ),
                Err(e) => check("database", Err(e.to_string())),
            },
            Err(e) => check("database", Err(e.to_string())),
        }
    }

    if failures > 0 {
        anyhow::bail!("{failures} check(s) failed");
    }
    println!("All checks passed.");
    Ok(())
}

fn secrets_check(data_dir: &Path) -> Result<String, String> {
    let path = tgeye_config::secrets_path(data_dir);
    if !path.exists() {
        return Ok("no secrets.toml (token may come from env)".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable by others (mode {:o}) — run `chmod 600`",
                path.display(),
                mode & 0o777
            ));
        }
        Ok(format!("{} (mode 600)", path.display()))
    }
    #[cfg(not(unix))]
    Ok(path.display().to_string())
}
