//! The PNS selection-mechanism config (plan6 D1): the TOML surface behind
//! `proofdb_harvest --pns-config <file>`. Defaults compiled in; a config
//! file sets any subset (unknown keys rejected, values validated); the
//! effective config is echoed on the session-start `pns:` line (plan6
//! §4.7). Mechanism-side semantics are normative in `selector/decision.rs`.
//!
//! **Degenerate configs are exact legacy policies** (the §2 equivalence
//! contract): `reserve_share = 0` (no rung ever fires) with
//! `layer_visit_cap = 0` (no layer rationing) reduces the selection to the
//! plan4 selector verbatim — verified by arm A′'s replay and the
//! decision-table tests.
//!
//! Knobs (plan6 §2): `reserve_share` — the fraction of the session cap
//! usable by rung visits (0 = no rungs; session cap 0 = unlimited reserve;
//! **compiled default 0.0** — the plan6 A/B measured the ladder a no-go,
//! §4.5/report6; set 0.25 via `--pns-config` to re-enable arm B).
//! `layer_visit_cap` — max expansion visits per ply layer per session
//! (0 = unlimited; rungs never count against a layer's cap).
//! `interleave_k` — every k-th visit (zero-based `V ≡ 0 mod k`; both visit
//! kinds count into `V`) considers a rung. `eligibility` —
//! `"no-virgin-child"` | `"always"`. `rung_growth` — rung budget by 1-based
//! visit rung `k`: `"geometric"` (`2^(k-1) × base`), `"linear"` (`k ×
//! base`), `"constant"` (`base`); expansion visits always use the plan4
//! geometric ladder regardless. `max_rung_passes` — rung visits per node
//! per session. `rotation` — `"fewest-passes"` (rotation, ties
//! `(ply, path)`) | `"number-ply-path"` (the queue order).
//!
//! No wall clock; parsing is total and deterministic. Tests in
//! `config/tests.rs`.

use serde::Deserialize;

/// When a censored node earns a rung revisit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    /// Censored and no virgin child (plan6 §2 insight 1).
    NoVirginChild,
    /// Every censored node is eligible.
    Always,
}

impl Eligibility {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoVirginChild => "no-virgin-child",
            Self::Always => "always",
        }
    }

    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "no-virgin-child" => Ok(Self::NoVirginChild),
            "always" => Ok(Self::Always),
            other => Err(format!(
                "unknown eligibility {other:?} (expected no-virgin-child | always)"
            )),
        }
    }
}

/// The rung-visit budget as a function of the 1-based visit rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RungGrowth {
    /// `2^(k-1) × base` (the plan4 ladder).
    Geometric,
    /// `k × base`.
    Linear,
    /// `base`.
    Constant,
}

impl RungGrowth {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Geometric => "geometric",
            Self::Linear => "linear",
            Self::Constant => "constant",
        }
    }

    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "geometric" => Ok(Self::Geometric),
            "linear" => Ok(Self::Linear),
            "constant" => Ok(Self::Constant),
            other => Err(format!(
                "unknown rung_growth {other:?} (expected geometric | linear | constant)"
            )),
        }
    }

    /// Budget for a rung visit at 1-based rung `k` (`k = passes + 1`;
    /// pass 1 is the original censored visit — rungs start at `k = 2`).
    #[must_use]
    pub fn budget(self, k: u32, base: u64) -> u64 {
        let k = u64::from(k);
        match self {
            Self::Geometric => {
                let e = k.saturating_sub(1).min(63);
                (1u64 << e).saturating_mul(base)
            }
            Self::Linear => k.saturating_mul(base),
            Self::Constant => base,
        }
    }
}

/// The ladder-target rotation among eligible nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// Fewest passes first (rotation — no node climbs while others wait);
    /// ties by `(ply, path)`.
    FewestPasses,
    /// The queue order `(effective number, ply, path)`.
    NumberPlyPath,
}

impl Rotation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FewestPasses => "fewest-passes",
            Self::NumberPlyPath => "number-ply-path",
        }
    }

    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "fewest-passes" => Ok(Self::FewestPasses),
            "number-ply-path" => Ok(Self::NumberPlyPath),
            other => Err(format!(
                "unknown rotation {other:?} (expected fewest-passes | number-ply-path)"
            )),
        }
    }
}

/// The effective PNS selection config (plan6 §2 defaults compiled in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PnsConfig {
    /// Fraction of the session cap usable by rung visits.
    pub reserve_share: f64,
    /// Max expansion visits per ply layer per session (0 = unlimited).
    pub layer_visit_cap: u64,
    /// Every k-th visit considers a rung (≥ 1).
    pub interleave_k: u64,
    pub eligibility: Eligibility,
    pub rung_growth: RungGrowth,
    /// Rung visits per node per session.
    pub max_rung_passes: u32,
    pub rotation: Rotation,
}

impl Default for PnsConfig {
    /// The plan6 §2 defaults table **as amended by the measured A/B verdict**
    /// (§4.5 decision rule, report6): the ladder is a measured no-go (9
    /// reserve-bound rungs, all censored, no depth dividend vs the
    /// ladder-free arm C), so the shipped default is rationed expansion
    /// only — `reserve_share = 0.0` (arm C's config); the rung mechanism
    /// stays available via `--pns-config` for later batches.
    fn default() -> Self {
        Self {
            reserve_share: 0.0,
            layer_visit_cap: 24,
            interleave_k: 4,
            eligibility: Eligibility::NoVirginChild,
            rung_growth: RungGrowth::Geometric,
            max_rung_passes: 3,
            rotation: Rotation::FewestPasses,
        }
    }
}

/// The plan6 §2 mechanism defaults **as pre-registered** (`reserve_share
/// 0.25`, arm B of the A/B). The compiled [`Default`] is arm C's (the
/// measured no-go verdict); this preset keeps the pre-registered mechanism
/// reachable by name.
#[must_use]
pub fn mechanism_defaults() -> PnsConfig {
    PnsConfig {
        reserve_share: 0.25,
        ..PnsConfig::default()
    }
}

/// The arm-C "ladder deletion" config (the §4 decision rule's alternative):
/// identical expansion behavior to the defaults minus rungs.
#[must_use]
pub fn ladder_deleted() -> PnsConfig {
    PnsConfig {
        reserve_share: 0.0,
        ..PnsConfig::default()
    }
}

/// The arm-A′ degenerate config: the exact plan4 selector (§2 equivalence
/// contract).
#[must_use]
pub fn degenerate() -> PnsConfig {
    PnsConfig {
        reserve_share: 0.0,
        layer_visit_cap: 0,
        ..PnsConfig::default()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    reserve_share: Option<f64>,
    layer_visit_cap: Option<u64>,
    interleave_k: Option<u64>,
    eligibility: Option<String>,
    rung_growth: Option<String>,
    max_rung_passes: Option<u32>,
    rotation: Option<String>,
}

impl PnsConfig {
    /// Parse the config file at `path` (TOML; unknown keys rejected; any
    /// subset may be present — absent keys keep their defaults).
    ///
    /// # Errors
    /// I/O, a TOML/serde error (incl. unknown keys), or validation.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read pns config {}: {e}", path.display()))?;
        let raw: RawConfig = toml::de::from_str(&text)
            .map_err(|e| format!("pns config {} is not valid: {e}", path.display()))?;
        let mut cfg = Self::default();
        if let Some(x) = raw.reserve_share {
            cfg.reserve_share = x;
        }
        if let Some(x) = raw.layer_visit_cap {
            cfg.layer_visit_cap = x;
        }
        if let Some(x) = raw.interleave_k {
            cfg.interleave_k = x;
        }
        if let Some(x) = &raw.eligibility {
            cfg.eligibility = Eligibility::parse(x)?;
        }
        if let Some(x) = &raw.rung_growth {
            cfg.rung_growth = RungGrowth::parse(x)?;
        }
        if let Some(x) = raw.max_rung_passes {
            cfg.max_rung_passes = x;
        }
        if let Some(x) = &raw.rotation {
            cfg.rotation = Rotation::parse(x)?;
        }
        cfg.validate()
            .map_err(|e| format!("pns config {} rejected: {e}", path.display()))?;
        Ok(cfg)
    }

    /// Range validation (file-loaded configs; the compiled defaults and
    /// the presets are valid by construction).
    ///
    /// # Errors
    /// A `reserve_share` outside `[0, 1]` (or non-finite), or an
    /// `interleave_k` of 0 (modulo-by-zero in the pacing check).
    pub fn validate(&self) -> Result<(), String> {
        if !self.reserve_share.is_finite() || !(0.0..=1.0).contains(&self.reserve_share) {
            return Err(format!(
                "reserve_share {} outside [0, 1]",
                self.reserve_share
            ));
        }
        if self.interleave_k == 0 {
            return Err("interleave_k must be ≥ 1".to_string());
        }
        Ok(())
    }

    /// The config echo (appended to the session-start `pns:` line).
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "cfg reserve_share {}, layer_visit_cap {}, interleave_k {}, \
             eligibility {}, rung_growth {}, max_rung_passes {}, rotation {}",
            self.reserve_share,
            self.layer_visit_cap,
            self.interleave_k,
            self.eligibility.as_str(),
            self.rung_growth.as_str(),
            self.max_rung_passes,
            self.rotation.as_str(),
        )
    }
}

#[cfg(test)]
mod tests;
