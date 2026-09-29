//! Offline shard export for the proofdb harvest loop (plan2 D2).
//!
//! Attribution: adapted from the campaign worker's `export_via_reconstruction`
//! (`examples/campaign_worker.rs`, the plan5/plan9 frozen prototype); that
//! file stays untouched — this small duplication is accepted and recorded in
//! the plan2 report (dedupe only if the campaign work reopens).
//!
//! Pipeline: TT snapshot → `reconstruct` (deterministic hole-filling over the
//! solved map) → `validate_proof_tree`. Returns the validated [`ProofTree`]
//! only; with a retained TT a raw DF-PN event stream is not always
//! self-contained (holes at nodes proven under earlier jobs), so only a
//! reconstruction-verified subtree crosses the shard boundary.

use std::io::Cursor;

use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct};
use atomic_solver::search::dfpn::Search;
use atomic_solver::tt_snapshot::{read_tt_snapshot, write_tt_snapshot};

/// Export the current search's decisive subtree at `sub_fen` as a validated
/// proof tree. Fails (no tree) if the snapshot round-trip, the
/// reconstruction, or the replay validator rejects it — the caller must not
/// write any shard or manifest entry.
///
/// # Errors
/// A message describing which pipeline stage rejected the subtree.
pub fn export_validated_shard(
    search: &Search,
    sub_fen: &str,
    config: &ReconstructConfig,
) -> Result<ProofTree, String> {
    let mut snap = Vec::new();
    write_tt_snapshot(
        search.tt(),
        sub_fen,
        config.tt_mb.min(u32::MAX as usize) as u32,
        &mut snap,
    )
    .map_err(|e| format!("snapshot write: {e}"))?;
    let (_header, solved, _unsolved) =
        read_tt_snapshot(&mut Cursor::new(&snap)).map_err(|e| format!("snapshot read: {e}"))?;
    let out = reconstruct(sub_fen, &solved, config);
    if out.root_outcome.is_none() {
        return Err(out
            .error
            .unwrap_or_else(|| "reconstruction did not resolve the root".to_string()));
    }
    let tree = out.tree.ok_or_else(|| {
        out.error
            .unwrap_or_else(|| "reconstruction produced no tree".to_string())
    })?;
    if let Err(defects) = validate_proof_tree(&tree) {
        return Err(format!(
            "replay validation found {} defects: {}",
            defects.len(),
            defects
                .iter()
                .take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    Ok(tree)
}
