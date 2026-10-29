//! Single source for the benchmark-fixture parser.
//!
//! Example binaries and integration tests cannot share a library module, so
//! both sides include this file by path: `examples/common.rs` via
//! `#[path = "../tests/common/fixture_parse.rs"]` and `tests/common/mod.rs`
//! via the ordinary child-module rule. The `include_str!` paths below resolve
//! relative to *this* file, so both users read `tests/fixtures/` and neither
//! copy needs editing when a fixture gains a field.

use atomic_solver::position::Outcome;

/// Fixture containing the move-order benchmark positions (m20 to m29).
pub const MOVE_ORDER_FIXTURE: &str = include_str!("../fixtures/move_order_positions.txt");

/// Fixture containing the decisive benchmark positions (dec01 to dec46).
pub const DECISIVE_FIXTURE: &str = include_str!("../fixtures/decisive_positions.txt");

/// A single move-order benchmark entry.
#[derive(Debug, Clone)]
pub struct MoveOrderCase {
    pub name: String,
    pub fen: String,
    pub expected: Option<Outcome>,
    pub note: Option<String>,
}

/// Load the move-order benchmark suite from the embedded fixture.
#[must_use]
pub fn load_move_order_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(MOVE_ORDER_FIXTURE)
}

/// Load the decisive benchmark suite from the embedded fixture.
#[must_use]
pub fn load_decisive_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(DECISIVE_FIXTURE)
}

/// Parse a fixture body: one `name; fen; expected; note` line per case, `#`
/// comments and blank lines skipped, empty `expected`/`note` fields as `None`.
pub fn parse_move_order_fixture(s: &str) -> Vec<MoveOrderCase> {
    s.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(4, ';');
            let name = parts.next().unwrap_or("").trim().to_string();
            let fen = parts.next().unwrap_or("").trim().to_string();
            let expected = parts.next().and_then(|p| {
                let p = p.trim();
                if p.is_empty() {
                    None
                } else {
                    p.parse::<Outcome>().ok()
                }
            });
            let note = parts
                .next()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from);
            MoveOrderCase {
                name,
                fen,
                expected,
                note,
            }
        })
        .collect()
}
