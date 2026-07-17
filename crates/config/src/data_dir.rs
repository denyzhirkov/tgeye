use std::path::{Path, PathBuf};

pub const DATA_DIR_NAME: &str = ".tgeye";
pub const HOME_ENV: &str = "TGEYE_HOME";

/// Project-local data dir (like tsk/kungfu): `--data-dir` → `TGEYE_HOME` → `./.tgeye`.
pub fn resolve_data_dir(
    cli: Option<PathBuf>,
    env: impl Fn(&str) -> Option<String>,
    cwd: &Path,
) -> PathBuf {
    if let Some(dir) = cli {
        return dir;
    }
    if let Some(home) = env(HOME_ENV).filter(|v| !v.trim().is_empty()) {
        return PathBuf::from(home);
    }
    cwd.join(DATA_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_flag_wins() {
        let dir = resolve_data_dir(
            Some(PathBuf::from("/custom")),
            |_| Some("/from-env".into()),
            Path::new("/cwd"),
        );
        assert_eq!(dir, PathBuf::from("/custom"));
    }

    #[test]
    fn env_beats_default() {
        let dir = resolve_data_dir(
            None,
            |k| (k == HOME_ENV).then(|| "/from-env".into()),
            Path::new("/cwd"),
        );
        assert_eq!(dir, PathBuf::from("/from-env"));
    }

    #[test]
    fn defaults_to_cwd_dot_tgeye() {
        let dir = resolve_data_dir(None, |_| None, Path::new("/cwd"));
        assert_eq!(dir, PathBuf::from("/cwd/.tgeye"));
    }

    #[test]
    fn blank_env_is_ignored() {
        let dir = resolve_data_dir(None, |_| Some("  ".into()), Path::new("/cwd"));
        assert_eq!(dir, PathBuf::from("/cwd/.tgeye"));
    }
}
