//! Command-line argument parsing for the atomic solver.
//!
//! This module is intentionally decoupled from the solver so that the same
//! parsing logic can be compiled into both the library (for unit tests) and the
//! binary. It only depends on `std`.
//!
//! This file is larger than 10 KiB because it doubles as the parser's test
//! suite (one test per option and error path).
//!
//! `STARTPOS_FEN` intentionally mirrors `Position::STARTPOS_FEN`: this module
//! is compiled in two crate roots (`#[cfg(test)] mod cli;` in the library and
//! `mod cli;` in the binary), and no single path resolves in both. The
//! equality is pinned by the `startpos_matches_position_starpos` test.

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
    /// Disable the detector-gated bounded pre-phase (plan13) — the
    /// region-closure fixpoint that decides small-space positions before the
    /// DF-PN loop. The ordinary DF-PN search then runs on every position. On
    /// such positions the pre-phase allocates a transient closure of up to
    /// 1,000,000 packed positions (~85 MB worst case) — the documented
    /// bounded exception to "RAM = TT only".
    pub no_preflight: bool,
    /// TT bucket-index salt. `0` (default) keeps shipped indexing; `s > 0`
    /// remaps which keys share a bucket without changing search semantics
    /// (noise-channel knob for the statistical gate).
    pub salt: u64,
    /// Child-evaluation budget for the whole search. `None` (default) means
    /// unbounded; a run that exhausts the budget returns `Draw` and reports
    /// `budget exhausted`.
    pub budget: Option<u64>,
    /// Assign provari-style move-class starting proof numbers to newly seen
    /// OR-node children (checks by evasions, double threat 1, threat 4, weak
    /// threat 8, quiet 9, hanging 15). Default `false`; an experimental
    /// search-shaping heuristic, not a correctness switch.
    pub move_class_pn: bool,
    /// Hide OR-node plain moves behind one delayed quiet-move group, opened
    /// only when the search selects it. Default `false`; an experimental
    /// search-shaping heuristic, not a correctness switch.
    pub quiet_grouping: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            fen: STARTPOS_FEN.to_string(),
            // 256 MB label: sizing rounds down to a power-of-two bucket
            // count (lean plan15 #20), so this label allocates 224 MiB —
            // byte-identical to the former round-up 128 MB default. The
            // previous default bump history: 64 MB was capacity-bound on
            // hard searches (#16, plan4; m22-class fills ~1M entries for
            // 2.4M nodes; doubling cut first-outcome work 2.6x — see
            // docs/plans/lean/research_tt_capacity.md), and the second bump
            // to 256 is label-honesty bookkeeping, not a capacity change.
            tt_size: 256,
            epsilon: 0.125,
            timeout: 5,
            first_outcome: false,
            refine_cap: 0.25,
            outcome_only: false,
            tt_dump_path: None,
            config_path: None,
            no_preflight: false,
            salt: 0,
            budget: None,
            move_class_pn: false,
            quiet_grouping: false,
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

/// Take the value of the option at `args[*i]` and advance past it.
fn value<'a>(args: &'a [String], i: &mut usize, name: &str) -> Result<&'a str, String> {
    let v = args
        .get(*i + 1)
        .ok_or_else(|| format!("error: {name} requires a value"))?;
    *i += 2;
    Ok(v.as_str())
}

/// Parse the value of the option at `args[*i]` as `T`.
fn number<T>(args: &[String], i: &mut usize, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value(args, i, name)?
        .parse::<T>()
        .map_err(|e| format!("error: invalid {name} value: {e}"))
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
        match args[i].as_str() {
            "-h" | "--help" => return Ok(ParseResult::Help),
            "--fen" => opts.fen = value(args, &mut i, "--fen")?.to_owned(),
            "--tt-size" => {
                let v: usize = number(args, &mut i, "--tt-size")?;
                if v == 0 {
                    return Err(format!("error: --tt-size must be positive, got {v}"));
                }
                opts.tt_size = v;
            }
            "--epsilon" => {
                let v: f64 = number(args, &mut i, "--epsilon")?;
                if !(0.0..=1.0).contains(&v) {
                    return Err(format!("error: epsilon must be in [0.0, 1.0], got {v}"));
                }
                opts.epsilon = v;
            }
            "--timeout" => {
                let v: u64 = number(args, &mut i, "--timeout")?;
                if v == 0 {
                    return Err(format!("error: timeout must be positive, got {v}"));
                }
                opts.timeout = v;
            }
            "--first-outcome" => {
                opts.first_outcome = true;
                i += 1;
            }
            "--refine-cap" => {
                let v: f64 = number(args, &mut i, "--refine-cap")?;
                if v < 0.0 {
                    return Err(format!("error: --refine-cap must be non-negative, got {v}"));
                }
                opts.refine_cap = v;
            }
            "--outcome-only" => {
                opts.outcome_only = true;
                i += 1;
            }
            "--tt-dump-path" => {
                opts.tt_dump_path = Some(value(args, &mut i, "--tt-dump-path")?.to_owned());
            }
            "--config" => {
                opts.config_path = Some(value(args, &mut i, "--config")?.to_owned());
            }
            "--no-preflight" => {
                opts.no_preflight = true;
                i += 1;
            }
            "--salt" => opts.salt = number(args, &mut i, "--salt")?,
            "--budget" => {
                let v: u64 = number(args, &mut i, "--budget")?;
                if v == 0 {
                    return Err(format!("error: --budget must be positive, got {v}"));
                }
                opts.budget = Some(v);
            }
            "--move-class-pn" => {
                opts.move_class_pn = true;
                i += 1;
            }
            "--quiet-grouping" => {
                opts.quiet_grouping = true;
                i += 1;
            }
            arg => {
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

    /// Parse `list` (option arguments only; the program name is prepended)
    /// and return the options, panicking on help or error.
    fn options(list: &[&str]) -> CliOptions {
        let mut argv = vec!["atomic_solver".to_string()];
        argv.extend(args(list));
        match parse_args(&argv).expect("parse ok") {
            ParseResult::Options(o) => o,
            ParseResult::Help => panic!("unexpected help"),
        }
    }

    /// The CLI keeps its own `STARTPOS_FEN` because this module is compiled in
    /// two crate roots (lib-test and bin); pin the two constants together so
    /// they cannot drift.
    #[test]
    fn startpos_matches_position_starpos() {
        let position_starpos = atomic_solver::position::Position::STARTPOS_FEN;
        assert_eq!(STARTPOS_FEN, position_starpos);
    }

    #[test]
    fn defaults_match_documentation() {
        let o = options(&[]);
        assert_eq!(o.fen, STARTPOS_FEN);
        assert_eq!(o.tt_size, 256);
        assert_eq!(o.epsilon, 0.125);
        assert_eq!(o.timeout, 5);
        assert!(!o.first_outcome);
        assert_eq!(o.refine_cap, 0.25);
        assert!(!o.outcome_only);
        assert!(o.tt_dump_path.is_none());
        assert!(o.config_path.is_none());
        assert!(!o.no_preflight);
        assert_eq!(o.salt, 0);
        assert!(o.budget.is_none());
        assert!(!o.move_class_pn);
        assert!(!o.quiet_grouping);
    }

    #[test]
    fn help_short_and_long() {
        for list in [
            &["-h"][..],
            &["--help"],
            &["atomic_solver", "-h"],
            &["atomic_solver", "--help"],
        ] {
            assert_eq!(parse_args(&args(list)).unwrap(), ParseResult::Help);
        }
    }

    #[test]
    fn fen_is_parsed() {
        let o = options(&["--fen", "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"]);
        assert_eq!(o.fen, "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1");
    }

    #[test]
    fn tt_size_is_parsed() {
        assert_eq!(options(&["--tt-size", "128"]).tt_size, 128);
    }

    #[test]
    fn epsilon_is_parsed() {
        assert_eq!(options(&["--epsilon", "0.5"]).epsilon, 0.5);
    }

    #[test]
    fn timeout_is_parsed() {
        assert_eq!(options(&["--timeout", "10"]).timeout, 10);
    }

    #[test]
    fn tt_dump_path_is_parsed() {
        let o = options(&["--tt-dump-path", "/tmp/tt.bin"]);
        assert_eq!(o.tt_dump_path, Some("/tmp/tt.bin".to_string()));
    }

    #[test]
    fn config_path_is_parsed() {
        let o = options(&["--config", "/tmp/scorer.toml"]);
        assert_eq!(o.config_path, Some("/tmp/scorer.toml".to_string()));
    }

    #[test]
    fn first_outcome_is_parsed() {
        assert!(options(&["--first-outcome"]).first_outcome);
    }

    #[test]
    fn refine_cap_is_parsed() {
        assert_eq!(options(&["--refine-cap", "0.5"]).refine_cap, 0.5);
    }

    #[test]
    fn refine_cap_zero_is_parsed() {
        assert_eq!(options(&["--refine-cap", "0"]).refine_cap, 0.0);
    }

    #[test]
    fn negative_refine_cap_rejected() {
        assert!(parse_args(&args(&["atomic_solver", "--refine-cap", "-0.1"])).is_err());
    }

    #[test]
    fn outcome_only_is_parsed() {
        assert!(options(&["--outcome-only"]).outcome_only);
    }

    #[test]
    fn no_preflight_is_parsed() {
        assert!(options(&["--no-preflight"]).no_preflight);
    }

    #[test]
    fn salt_is_parsed() {
        assert_eq!(options(&["--salt", "42"]).salt, 42);
        // Salt 0 is explicit and valid (identity).
        assert_eq!(options(&["--salt", "0"]).salt, 0);
        // u64::MAX must round-trip.
        assert_eq!(options(&["--salt", "18446744073709551615"]).salt, u64::MAX);
    }

    #[test]
    fn salt_rejects_non_numeric() {
        assert!(parse_args(&args(&["atomic_solver", "--salt", "x"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--salt", "-1"])).is_err());
    }

    #[test]
    fn budget_is_parsed() {
        let o = options(&["--budget", "1000000000"]);
        assert_eq!(o.budget, Some(1_000_000_000));
    }

    #[test]
    fn move_class_pn_is_parsed_and_default_off() {
        assert!(!options(&[]).move_class_pn);
        assert!(options(&["--move-class-pn"]).move_class_pn);
    }

    #[test]
    fn quiet_grouping_is_parsed_and_default_off() {
        assert!(!options(&[]).quiet_grouping);
        assert!(options(&["--quiet-grouping"]).quiet_grouping);
    }

    #[test]
    fn budget_rejects_non_positive_and_non_numeric() {
        assert!(parse_args(&args(&["atomic_solver", "--budget", "0"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--budget", "-5"])).is_err());
        assert!(parse_args(&args(&["atomic_solver", "--budget", "x"])).is_err());
    }

    #[test]
    fn missing_value_returns_err() {
        for opt in [
            "--fen",
            "--tt-size",
            "--epsilon",
            "--timeout",
            "--tt-dump-path",
            "--config",
            "--refine-cap",
            "--salt",
            "--budget",
        ] {
            assert!(parse_args(&args(&["atomic_solver", opt])).is_err(), "{opt}");
        }
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
