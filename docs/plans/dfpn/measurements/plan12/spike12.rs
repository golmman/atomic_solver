//! TEMPORARY plan12 Phase 0 work-mass diagnostics (T2) — spike code, gated on
//! `DFPN12_SPIKE=1`, reverted after measuring. Source is archived under
//! `docs/plans/dfpn/measurements/plan12/` together with the raw logs.
//!
//! Counters (plan12 T2 items; see `docs/plans/dfpn/plan12.md`):
//!
//! 1. TT store suppression: suppressed solved-draw stores per run + distinct
//!    positions covered.
//! 2. plan9 repetition cache: probes/hits/misses, distinct (hash, context)
//!    keys, ancestor-context size distribution at store and probe, and
//!    same-node/different-context fragmentation (distinct positions stored
//!    under ≥ 2 distinct contexts; probe misses against a stored
//!    different-context entry).
//! 3. Path-repetition cut frequency: share of `dfpn` frames exiting via
//!    `path_contains`.
//! 4. Child-eval mass by (OR/AND × resolved/unsolved) and by parent frame
//!    depth bucket.
//! 5. Clock fragmentation (R3): probe misses where the same (repetition key,
//!    context hash) pair was seen earlier in the run at a different full
//!    position hash.
//!
//! R1 attribution (outermost-cause, child-eval units): every evaluated child
//! is counted exactly once, to the outermost applicable mechanism:
//! - class 1: inside the subtree of a frame whose solved-draw result was
//!   repetition-suppressed (contaminated region),
//! - class 2: inside the subtree of a frame whose repetition-cache probe
//!   missed while a same-node/different-context or same-rep-key/
//!   different-clock entry existed (fragmentation),
//! - class 3: residual.
//!
//! Implementation: frames push a record at entry; at exit a frame attributes
//! its subtree's *unattributed* evals (`subtree − descendants' attributions −
//! live pending`), so nested mechanisms never double count and class 1 wins
//! over class 2 by truncating descendant pendings at a suppressed frame's
//! exit. Fragmentation frames defer their attribution as "pending" values,
//! resolved to class 2 at report time — a pending inside a suppressed region
//! was already folded into that region's class-1 mass by the truncation.

#![allow(clippy::struct_field_names)]

use std::collections::{HashMap, HashSet};

use super::Search;

pub(super) struct SpikeFrame {
    evals_start: u64,
    attributed_at_entry: u64,
    pending_at_entry: u64,
    frag_len_at_entry: usize,
    frag: bool,
}

const HIST_BUCKETS: usize = 8;

pub(super) fn depth_bucket(d: usize) -> usize {
    match d {
        0 => 0,
        1 => 1,
        2..=4 => 2,
        5..=8 => 3,
        9..=16 => 4,
        17..=32 => 5,
        33..=64 => 6,
        _ => 7,
    }
}

#[derive(Default)]
pub(super) struct Spike12 {
    // item 1: TT store suppression
    pub suppress_stores: u64,
    pub suppress_keys: HashSet<u64>,
    // item 2: plan9 repetition cache
    pub cache_probes: u64,
    pub cache_hits: u64,
    pub store_ctx_hist: [u64; HIST_BUCKETS],
    pub probe_ctx_hist: [u64; HIST_BUCKETS],
    pub stored_pairs: HashSet<(u64, u64)>,
    pub stored_pairs_capped: u64,
    pub multi_ctx_positions: HashSet<u64>,
    pub multi_ctx_capped: u64,
    pub tt_first_ctx: HashMap<u64, u64>,
    pub ctx_frag_misses: u64,
    // item 5: clock fragmentation
    pub seen_pairs: HashMap<(u64, u64), u64>,
    pub seen_pairs_capped: u64,
    pub clock_frag_misses: u64,
    pub clock_frag_pairs: HashSet<(u64, u64)>,
    pub clock_frag_pairs_capped: u64,
    // item 3: frame exits
    pub frames_total: u64,
    pub frames_terminal: u64,
    pub frames_leaf: u64,
    pub frames_at_rep_check: u64,
    pub frames_path_rep: u64,
    pub frames_tt_resolved: u64,
    pub frames_cache_hit: u64,
    // item 4: child-eval classification
    pub eval_or_resolved: u64,
    pub eval_or_unsolved: u64,
    pub eval_and_resolved: u64,
    pub eval_and_unsolved: u64,
    pub eval_resolved_rep: u64,
    pub eval_depth_hist: [u64; HIST_BUCKETS],
    // R1 attribution
    pub class1: u64,
    pub attributed_total: u64,
    pub frag_pending: Vec<u64>,
    pub pending_sum: u64,
    pub frames: Vec<SpikeFrame>,
    pub total_child_evals: u64,
}

const SET_CAP: usize = 2_000_000;

impl Spike12 {
    pub(super) fn frame_entry(&mut self, child_evals: u64) {
        self.frames_at_rep_check += 1;
        self.frames.push(SpikeFrame {
            evals_start: child_evals,
            attributed_at_entry: self.attributed_total,
            pending_at_entry: self.pending_sum,
            frag_len_at_entry: self.frag_pending.len(),
            frag: false,
        });
    }

    pub(super) fn frame_exit_early(&mut self, kind: u8) {
        match kind {
            0 => self.frames_path_rep += 1,
            1 => self.frames_tt_resolved += 1,
            _ => self.frames_cache_hit += 1,
        }
        self.frames.pop();
    }

    /// Fragmentation check at a cache probe miss; `true` if the frame must be
    /// marked class-2 eligible (same-node/different-context or
    /// same-rep-key/different-clock entry existed).
    pub(super) fn probe_miss(
        &mut self,
        tt_key: u64,
        rep_key: u64,
        context_hash: u64,
        path_len: usize,
    ) -> bool {
        self.cache_probes += 1;
        self.probe_ctx_hist[depth_bucket(path_len)] += 1;
        let mut frag = false;
        // item 2: same node stored under a different context.
        if let Some(&c0) = self.tt_first_ctx.get(&tt_key)
            && c0 != context_hash
        {
            self.ctx_frag_misses += 1;
            frag = true;
        }
        // item 5: same (rep key, context) seen earlier at a different full hash.
        let pair = (rep_key, context_hash);
        match self.seen_pairs.get(&pair) {
            Some(&t0) => {
                if t0 != tt_key {
                    self.clock_frag_misses += 1;
                    frag = true;
                    if self.clock_frag_pairs.len() < SET_CAP {
                        self.clock_frag_pairs.insert(pair);
                    } else {
                        self.clock_frag_pairs_capped += 1;
                    }
                }
            }
            None => {
                if self.seen_pairs.len() < SET_CAP {
                    self.seen_pairs.insert(pair, tt_key);
                } else {
                    self.seen_pairs_capped += 1;
                }
            }
        }
        frag
    }

    pub(super) fn probe_hit(&mut self, path_len: usize) {
        self.cache_probes += 1;
        self.cache_hits += 1;
        self.probe_ctx_hist[depth_bucket(path_len)] += 1;
    }

    pub(super) fn store_suppressed(&mut self, tt_key: u64, context_hash: u64, path_len: usize) {
        self.suppress_stores += 1;
        if self.suppress_keys.len() < SET_CAP {
            self.suppress_keys.insert(tt_key);
        }
        self.store_ctx_hist[depth_bucket(path_len)] += 1;
        if self.stored_pairs.len() < SET_CAP {
            self.stored_pairs.insert((tt_key, context_hash));
        } else {
            self.stored_pairs_capped += 1;
        }
        match self.tt_first_ctx.get(&tt_key) {
            Some(&c0) => {
                if c0 != context_hash && self.multi_ctx_positions.len() < SET_CAP {
                    self.multi_ctx_positions.insert(tt_key);
                } else if c0 != context_hash {
                    self.multi_ctx_capped += 1;
                }
            }
            None => {
                self.tt_first_ctx.insert(tt_key, context_hash);
            }
        }
    }

    pub(super) fn eval(&mut self, parent_is_or: bool, resolved: bool, rep_seen: bool, depth: usize) {
        match (parent_is_or, resolved) {
            (true, true) => self.eval_or_resolved += 1,
            (true, false) => self.eval_or_unsolved += 1,
            (false, true) => self.eval_and_resolved += 1,
            (false, false) => self.eval_and_unsolved += 1,
        }
        if resolved && rep_seen {
            self.eval_resolved_rep += 1;
        }
        self.eval_depth_hist[depth_bucket(depth)] += 1;
    }

    /// Attribution at the main frame exit (the only exit where child evals
    /// can have advanced). `suppressed` = the frame's solved draw was
    /// repetition-suppressed.
    pub(super) fn frame_exit_main(&mut self, child_evals: u64, suppressed: bool) {
        if let Some(f) = self.frames.pop() {
            let subtree = child_evals - f.evals_start;
            let nested_attr = self.attributed_total - f.attributed_at_entry;
            let pending_delta = self.pending_sum - f.pending_at_entry;
            let unattrib = subtree.saturating_sub(nested_attr).saturating_sub(pending_delta);
            if suppressed {
                self.class1 += unattrib;
                self.attributed_total += unattrib;
                self.frag_pending.truncate(f.frag_len_at_entry);
                self.pending_sum = f.pending_at_entry;
            } else if f.frag {
                self.frag_pending.push(unattrib);
                self.pending_sum += unattrib;
            }
        }
    }

    pub(super) fn summary(&self, total_child_evals: u64) -> String {
        let class2 = self.pending_sum;
        let class3 = total_child_evals
            .saturating_sub(self.class1)
            .saturating_sub(class2);
        let mut s = String::with_capacity(4096);
        let pct = |n: u64| {
            if total_child_evals > 0 {
                format!("{:.2}%", 100.0 * n as f64 / total_child_evals as f64)
            } else {
                "n/a".to_string()
            }
        };
        s.push_str("spike12: ---- work-mass diagnostics (plan12 T2) ----\n");
        s.push_str(&format!(
            "spike12: total_child_evals={total_child_evals} frames_total={} frames_terminal={} frames_leaf={} frames_at_rep_check={} frames_path_rep={} frames_tt_resolved={} frames_cache_hit={}\n",
            self.frames_total,
            self.frames_terminal,
            self.frames_leaf,
            self.frames_at_rep_check,
            self.frames_path_rep,
            self.frames_tt_resolved,
            self.frames_cache_hit
        ));
        if self.frames_at_rep_check > 0 {
            s.push_str(&format!(
                "spike12: item3 path_rep_exit_share={:.3}% ({}/{})\n",
                100.0 * self.frames_path_rep as f64 / self.frames_at_rep_check as f64,
                self.frames_path_rep,
                self.frames_at_rep_check
            ));
        }
        s.push_str(&format!(
            "spike12: item1 suppress_stores={} suppress_distinct_positions={}\n",
            self.suppress_stores,
            self.suppress_keys.len()
        ));
        s.push_str(&format!(
            "spike12: item2 cache_probes={} cache_hits={:.2}% stored_pairs={} (cap_rej={}) store_ctx_hist={:?} probe_ctx_hist={:?}\n",
            self.cache_probes,
            if self.cache_probes > 0 {
                100.0 * self.cache_hits as f64 / self.cache_probes as f64
            } else {
                0.0
            },
            self.stored_pairs.len(),
            self.stored_pairs_capped,
            self.store_ctx_hist,
            self.probe_ctx_hist
        ));
        s.push_str(&format!(
            "spike12: item2 ctx_fragmentation: multi_ctx_positions={} ctx_frag_misses={}\n",
            self.multi_ctx_positions.len(),
            self.ctx_frag_misses
        ));
        s.push_str(&format!(
            "spike12: item5 clock_fragmentation: misses={} distinct_pairs={} (cap_rej={} seen_pairs={} seen_cap_rej={})\n",
            self.clock_frag_misses,
            self.clock_frag_pairs.len(),
            self.clock_frag_pairs_capped,
            self.seen_pairs.len(),
            self.seen_pairs_capped
        ));
        s.push_str(&format!(
            "spike12: item4 evals: OR resolved={} unsolved={} | AND resolved={} unsolved={} | resolved_rep_draws={} | depth_hist={:?}\n",
            self.eval_or_resolved,
            self.eval_or_unsolved,
            self.eval_and_resolved,
            self.eval_and_unsolved,
            self.eval_resolved_rep,
            self.eval_depth_hist
        ));
        s.push_str(&format!(
            "spike12: R1 attribution: class1_contaminated={} ({}) class2_fragmentation={class2} ({}) class3_other={class3} ({})\n",
            self.class1,
            pct(self.class1),
            pct(class2),
            pct(class3)
        ));
        s
    }
}

impl Search {
    pub(super) fn spike12_frame_entry(&mut self) {
        if let Some(s) = &mut self.spike12 {
            s.frames_total += 1;
            let evals = self.child_evals;
            s.frame_entry(evals);
        }
    }

    pub(super) fn spike12_frame_terminal(&mut self) {
        if let Some(s) = &mut self.spike12 {
            s.frames_total += 1;
            s.frames_terminal += 1;
        }
    }

    pub(super) fn spike12_frame_leaf(&mut self) {
        if let Some(s) = &mut self.spike12 {
            s.frames_total += 1;
            s.frames_leaf += 1;
        }
    }

    pub(super) fn spike12_frame_exit_early(&mut self, kind: u8) {
        if let Some(s) = &mut self.spike12 {
            s.frame_exit_early(kind);
        }
    }

    pub(super) fn spike12_probe_hit(&mut self) {
        if let Some(s) = &mut self.spike12 {
            let len = self.path_stack.len();
            s.probe_hit(len);
        }
    }

    pub(super) fn spike12_probe_miss(&mut self, tt_key: u64, rep_key: u64, context_hash: u64) {
        if let Some(s) = &mut self.spike12 {
            let len = self.path_stack.len();
            let frag = s.probe_miss(tt_key, rep_key, context_hash, len);
            if frag
                && let Some(f) = s.frames.last_mut()
            {
                f.frag = true;
            }
        }
    }

    pub(super) fn spike12_store_suppressed(&mut self, tt_key: u64, context_hash: u64) {
        if let Some(s) = &mut self.spike12 {
            let len = self.path_stack.len();
            s.store_suppressed(tt_key, context_hash, len);
        }
    }

    pub(super) fn spike12_frame_exit_main(&mut self, suppressed: bool) {
        if let Some(s) = &mut self.spike12 {
            let evals = self.child_evals;
            s.frame_exit_main(evals, suppressed);
        }
    }

    pub(super) fn spike12_eval(&mut self, parent_is_or: bool, resolved: bool, rep_seen: bool) {
        if let Some(s) = &mut self.spike12 {
            let depth = self.path_stack.len();
            s.eval(parent_is_or, resolved, rep_seen, depth);
        }
    }
}
