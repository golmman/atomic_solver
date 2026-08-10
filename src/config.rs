//! Configuration loader for the atomic-chess solver.
//!
//! Currently only the `ScorerParams` used by `StaticAtomicScorer` are loaded
//! from an external TOML file. Future parameters can be added here.

use std::path::Path;

use serde::Deserialize;

use crate::search::ordering::{ScorerParams, ScorerParamsError};

/// Error that can occur while loading or validating a config file.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Invalid(ScorerParamsError),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read config file: {e}"),
            Self::Parse(e) => write!(f, "failed to parse config file: {e}"),
            Self::Invalid(e) => write!(f, "invalid scorer parameters: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
struct ConfigFile {
    scorer: ScorerParams,
}

/// Load and validate a `ScorerParams` from a TOML file at `path`.
pub fn load_scorer_config<P: AsRef<Path>>(path: P) -> Result<ScorerParams, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    let config: ConfigFile = toml::de::from_str(&contents).map_err(ConfigError::Parse)?;
    config.scorer.validate().map_err(ConfigError::Invalid)?;
    Ok(config.scorer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_TOML: &str = r#"
[scorer]
score_winning_capture = 100_000_000
score_promotion = 1_000_000
score_capture = 5_000
capture_net_scale = 10
score_threat_last = 10_000
score_threat = 1_000
score_kamikaze_last = 9_000
score_kamikaze = 3_000
score_approach = 100
score_approach_step = 10
score_center = 50
score_center_step = 10
score_pawn_storm = 5_500
score_pawn_storm_step = 100
score_rook_center = 500
score_rook_open_file = 2_000
score_rook_open_file_step = 50
score_rook_back_rank = 300
and_pawn_storm_scale = 50
and_rook_attack_scale = 50
and_approach_scale = 75

[scorer.pieces]
pawn = 100
knight = 320
bishop = 330
rook = 500
queen = 900
commoner = 20_000
"#;

    fn with_temp_config(contents: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("atomic_solver_config_test_{}_{n}.toml", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, contents).expect("write temp config");
        path
    }

    #[test]
    fn missing_config_file_returns_io_error() {
        let result = load_scorer_config("/nonexistent/config.toml");
        assert!(matches!(result, Err(ConfigError::Io(_))));
    }

    #[test]
    fn valid_config_loads_and_validates() {
        let path = with_temp_config(DEFAULT_TOML);
        let params = load_scorer_config(&path).expect("valid config should load");
        let _ = std::fs::remove_file(&path);
        assert_eq!(params.score_winning_capture, 100_000_000);
        assert_eq!(params.pieces.commoner, 20_000);
    }

    #[test]
    fn invalid_config_rejected_by_validation() {
        let mut toml = DEFAULT_TOML.to_string();
        toml = toml.replace(
            "score_winning_capture = 100_000_000",
            "score_winning_capture = 100",
        );
        let path = with_temp_config(&toml);
        let result = load_scorer_config(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(result, Err(ConfigError::Invalid(_))));
    }
}
