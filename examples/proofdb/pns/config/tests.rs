//! `config.rs` unit tests (plan6 D1): the compiled defaults, growth
//! budgets, TOML parsing/validation, and the echo. Included by `config.rs`;
//! also runs via `tests/proofdb.rs`.

use super::*;

#[test]
fn defaults_are_the_plan6_table() {
    let c = PnsConfig::default();
    // The compiled default is arm C's config: the plan6 A/B measured the
    // ladder a no-go (§4.5 decision rule); see the Default impl docs.
    assert_eq!(c.reserve_share, 0.0);
    assert_eq!(c.layer_visit_cap, 24);
    assert_eq!(c.interleave_k, 4);
    assert_eq!(c.eligibility, Eligibility::NoVirginChild);
    assert_eq!(c.rung_growth, RungGrowth::Geometric);
    assert_eq!(c.max_rung_passes, 3);
    assert_eq!(c.rotation, Rotation::FewestPasses);
    c.validate().unwrap();
    // Arm B (the mechanism defaults as pre-registered in §2) is reachable
    // via an explicit reserve share.
    let arm_b = PnsConfig {
        reserve_share: 0.25,
        ..PnsConfig::default()
    };
    assert_eq!(arm_b.reserve_share, 0.25);
    assert_eq!(
        ladder_deleted(),
        PnsConfig::default(),
        "arm C's config IS the shipped default now"
    );
    assert_eq!(degenerate().layer_visit_cap, 0);
    assert_eq!(degenerate().reserve_share, 0.0);
}

#[test]
fn rung_growth_budgets() {
    let base = 4_000_000u64;
    // k = 1 (the original censored visit) is never a rung budget, but the
    // formula is total: all growth modes agree at k = 1.
    for g in [
        RungGrowth::Geometric,
        RungGrowth::Linear,
        RungGrowth::Constant,
    ] {
        assert_eq!(g.budget(1, base), base);
    }
    assert_eq!(RungGrowth::Geometric.budget(2, base), 2 * base);
    assert_eq!(RungGrowth::Geometric.budget(3, base), 4 * base);
    assert_eq!(RungGrowth::Geometric.budget(4, base), 8 * base);
    assert_eq!(RungGrowth::Linear.budget(2, base), 2 * base);
    assert_eq!(RungGrowth::Linear.budget(3, base), 3 * base);
    assert_eq!(RungGrowth::Constant.budget(4, base), base);
}

#[test]
fn toml_parse_and_reject() {
    let dir = std::env::temp_dir().join(format!("proofdb_cfg_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("arm_b.toml");
    std::fs::write(
        &p,
        "reserve_share = 0.25\nlayer_visit_cap = 24\ninterleave_k = 4\n\
         eligibility = \"no-virgin-child\"\nrung_growth = \"geometric\"\n\
         max_rung_passes = 3\nrotation = \"fewest-passes\"\n",
    )
    .unwrap();
    assert_eq!(PnsConfig::load(&p).unwrap(), mechanism_defaults());

    // Partial config: absent keys keep defaults.
    let p = dir.join("partial.toml");
    std::fs::write(&p, "reserve_share = 0.0\nlayer_visit_cap = 24\n").unwrap();
    let c = PnsConfig::load(&p).unwrap();
    assert_eq!(c.reserve_share, 0.0);
    assert_eq!(c.layer_visit_cap, 24);
    assert_eq!(c.interleave_k, 4);

    // Unknown key rejected.
    let p = dir.join("unknown.toml");
    std::fs::write(&p, "reserve_share = 0.5\nbogus = 1\n").unwrap();
    assert!(PnsConfig::load(&p).unwrap_err().contains("bogus"));

    // Bad values rejected.
    for (toml_text, expect) in [
        ("reserve_share = 1.5\n", "reserve_share"),
        ("reserve_share = -0.1\n", "reserve_share"),
        ("interleave_k = 0\n", "interleave_k"),
        ("eligibility = \"sometimes\"\n", "eligibility"),
        ("rung_growth = \"exponential\"\n", "rung_growth"),
        ("rotation = \"random\"\n", "rotation"),
    ] {
        let p = dir.join("bad.toml");
        std::fs::write(&p, toml_text).unwrap();
        let e = PnsConfig::load(&p).unwrap_err();
        assert!(e.contains(expect), "{toml_text:?} → {e}");
    }
    // Missing file.
    assert!(PnsConfig::load(&dir.join("absent.toml")).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn describe_is_a_total_echo() {
    let s = PnsConfig {
        reserve_share: 0.25,
        ..PnsConfig::default()
    }
    .describe();
    for part in [
        "reserve_share 0.25",
        "layer_visit_cap 24",
        "interleave_k 4",
        "eligibility no-virgin-child",
        "rung_growth geometric",
        "max_rung_passes 3",
        "rotation fewest-passes",
    ] {
        assert!(s.contains(part), "{s}");
    }
    assert!(PnsConfig::default().describe().contains("reserve_share 0"));
}
