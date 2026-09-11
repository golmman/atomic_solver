//! Command-line argument parsing for the atomic solver.
//!
//! This module is intentionally decoupled from the solver so that the same
//! parsing logic can be compiled into both the library (for unit tests) and the
//! binary. It only depends on `std`.
//!
//! This file is larger than 10 KiB because it contains both the parser and
//! the unit tests that exercise every option and error path.
//!
//! `STARTPOS_FEN` intentionally mirrors `crate::position::Position::STARTPOS_FEN`
//! so that the CLI default stays `std`-only and does not pull in the solver crate.

/// Parsed command-line options for the solver.
#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    /// FEN string to solve. Defaults to the standard atomic start position.
    pub fen: String,
    /// Transposition-table size in megabytes.
    pub tt_size: usize,
    /// DF-PN+ threshold parameter in the range `[0.0, 1.0]`.
    pub epsilon: f64,
    /// Search timeout in seconds.
    pub timeout: u64,
    /// Stop after the first decisive outcome and skip iterative PV refinement.
    pub first_outcome: bool,
    /// Per-refinement-round work-cap factor relative to the first-outcome
    /// phase's child evaluations. `0.0` disables capping.
    pub refine_cap: f64,
    /// Print only the outcome/PV: no stdin reader, no pre-exit summary.
    pub outcome_only: bool,
    /// Optional path for the binary TT snapshot written after the search.
    /// `None` disables snapshot writing (the default).
    pub tt_dump_path: Option<String>,
    /// Path to a TOML config file overriding the default scorer parameters.
    pub config_path: Option<String>,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            fen: STARTPOS_FEN.to_string(),
            // 128 MB: the 64 MB default was capacity-bound on hard searches
            // (m22-class fills ~1M entries for 2.4M nodes; doubling the table
            // cut first-outcome work 2.6x — see
            // docs/plans/lean/research_tt_capacity.md).
            tt_size: 128,
            epsilon: 0.125,
            timeout: 5,
            first_outcome: false,
            refine_cap: 0.25,
            outcome_only: false,
            tt_dump_path: None,
            config_path: None,
        }
    }
}

/// Default atomic start position.
pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Result of parsing the command-line arguments.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult {
    /// The user asked for help.
    Help,
    /// Parsed options.
    Options(CliOptions),
}

/// Parse the supplied command-line argument strings.
///
/// The first argument may be the program name and is ignored if it does not
/// start with `-`. Returns `Ok(ParseResult::Help)` for `-h`/`--help`,
/// `Ok(ParseResult::Options(_))` for valid options, and `Err(_)` for unknown
/// options, missing values, or out-of-range values.
pub fn parse_args(args: &[String]) -> Result<ParseResult, String> {
    let mut opts = CliOptions::default();
    let mut i = 0;
    if !args.is_empty() && !args[0].starts_with('-') {
        i = 1;
    }

    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => {
                return Ok(ParseResult::Help);
            }
            "--fen" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --fen requires a value".to_string())?;
                opts.fen = value.clone();
                i += 2;
            }
            "--tt-size" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --tt-size requires a value".to_string())?;
                let v = value
                    .parse::<usize>()
                    .map_err(|e| format!("error: invalid --tt-size value: {e}"))?;
                if v == 0 {
                    return Err(format!("error: --tt-size must be positive, got {v}"));
                }
                opts.tt_size = v;
                i += 2;
            }
            "--epsilon" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --epsilon requires a value".to_string())?;
                let v = value
                    .parse::<f64>()
                    .map_err(|e| format!("error: invalid epsilon value: {e}"))?;
                if !(0.0..=1.0).contains(&v) {
                    return Err(format!("error: epsilon must be in [0.0, 1.0], got {v}"));
                }
                opts.epsilon = v;
                i += 2;
            }
            "--timeout" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --timeout requires a value".to_string())?;
                let v = value
                    .parse::<u64>()
                    .map_err(|e| format!("error: invalid timeout value: {e}"))?;
                if v == 0 {
                    return Err(format!("error: timeout must be positive, got {v}"));
                }
                opts.timeout = v;
                i += 2;
            }
            "--first-outcome" => {
                opts.first_outcome = true;
                i += 1;
            }
            "--refine-cap" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --refine-cap requires a value".to_string())?;
                let v = value
                    .parse::<f64>()
                    .map_err(|e| format!("error: invalid --refine-cap value: {e}"))?;
                if v < 0.0 {
                    return Err(format!("error: --refine-cap must be non-negative, got {v}"));
                }
                opts.refine_cap = v;
                i += 2;
            }
            "--outcome-only" => {
                opts.outcome_only = true;
                i += 1;
            }
            "--tt-dump-path" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --tt-dump-path requires a value".to_string())?;
                opts.tt_dump_path = Some(value.clone());
                i += 2;
            }
            "--config" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "error: --config requires a value".to_string())?;
                opts.config_path = Some(value.clone());
                i += 2;
            }
            _ => {
                return Err(format!(
                    "error: unknown option '{arg}'\nRun with --help for usage."
                ));
            }
        }
    }

    Ok(ParseResult::Options(opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn defaults_match_documentation() {
        let parsed = parse_args(&args(&["atomic_solver"])).unwrap();
        let CliOptions {
            fen,
            tt_size,
            epsilon,
            timeout,
            first_outcome,
            refine_cap,
            outcome_only,
            tt_dump_path,
            config_path,
        } = match parsed {
            ParseResult::Options(o) => o,
            ParseResult::Help => panic!("unexpected help"),
        };
        assert_eq!(fen, STARTPOS_FEN);
        assert_eq!(tt_size, 128);
        assert_eq!(epsilon, 0.125);
        assert_eq!(timeout, 5);
        assert!(!first_outcome);
        assert_eq!(refine_cap, 0.25);
        assert!(!outcome_only);
        assert!(tt_dump_path.is_none());
        assert!(config_path.is_none());
    }

    #[test]
    fn help_short_and_long() {
        assert_eq!(parse_args(&args(&["-h"])).unwrap(), ParseResult::Help);
        assert_eq!(parse_args(&args(&["--help"])).unwrap(), ParseResult::Help);
        assert_eq!(
            parse_args(&args(&["atomic_solver", "-h"])).unwrap(),
            ParseResult::Help
        );
        assert_eq!(
            parse_args(&args(&["atomic_solver", "--help"])).unwrap(),
            ParseResult::Help
        );
    }

    #[test]
    fn fen_is_parsed() {
        let parsed = parse_args(&args(&[
            "atomic_solver",
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
        ]))
        .unwrap();
        match parsed {
            ParseResult::Options(o) => {
                assert_eq!(o.fen, "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1");
            }
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn tt_size_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--tt-size", "128"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.tt_size, 128),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn epsilon_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--epsilon", "0.5"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.epsilon, 0.5),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn timeout_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--timeout", "10"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.timeout, 10),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn tt_dump_path_is_parsed() {
        let parsed =
            parse_args(&args(&["atomic_solver", "--tt-dump-path", "/tmp/tt.bin"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.tt_dump_path, Some("/tmp/tt.bin".to_string())),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn config_path_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--config", "/tmp/scorer.toml"])).unwrap();
        match parsed {
            ParseResult::Options(o) => {
                assert_eq!(o.config_path, Some("/tmp/scorer.toml".to_string()))
            }
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn first_outcome_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--first-outcome"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert!(o.first_outcome),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn refine_cap_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--refine-cap", "0.5"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.refine_cap, 0.5),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn refine_cap_zero_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--refine-cap", "0"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert_eq!(o.refine_cap, 0.0),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn negative_refine_cap_rejected() {
        assert!(parse_args(&args(&["atomic_solver", "--refine-cap", "-0.1"])).is_err());
    }

    #[test]
    fn outcome_only_is_parsed() {
        let parsed = parse_args(&args(&["atomic_solver", "--outcome-only"])).unwrap();
        match parsed {
            ParseResult::Options(o) => assert!(o.outcome_only),
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn missing_value_returns_err() {
        assert!(parse_args(&args(&["atomic_solver", "--fen"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--tt-size"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--epsilon"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--timeout"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--tt-dump-path"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--config"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--refine-cap"])).is_err());
    }

    /// `--pt-size` / `--dump-path` were removed from the search CLI with plan7:
    /// the search no longer builds proof trees. Passing them must hit the
    /// unknown-option error path (this pins the removal against accidental
    /// re-adds).
    #[test]
    fn removed_proof_tree_options_rejected() {
        let err = parse_args(&args(&["atomic_solver", "--pt-size", "64"])).unwrap_err();
        assert!(err.contains("unknown option"), "got: {err}");
        let err = parse_args(&args(&["atomic_solver", "--dump-path", "x.bin"])).unwrap_err();
        assert!(err.contains("unknown option"), "got: {err}");
    }

    #[test]
    fn unknown_option_returns_err() {
        let err = parse_args(&args(&["atomic_solver", "--bogus"])).unwrap_err();
        assert!(err.contains("unknown option"));
    }

    #[test]
    fn non_positive_tt_size_rejected() {
        assert!(parse_args(&args(&["atomic_solver", "--tt-size", "0"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--tt-size", "-1"])).is_err());
    }

    #[test]
    fn non_positive_timeout_rejected() {
        assert!(parse_args(&args(&["atomic_solver", "--timeout", "0"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--timeout", "-1"])).is_err());
    }

    #[test]
    fn out_of_range_epsilon_rejected() {
        assert!(parse_args(&args(&["atomic_solver", "--epsilon", "-0.1"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--epsilon", "1.1"])).is_err());
    }
}
