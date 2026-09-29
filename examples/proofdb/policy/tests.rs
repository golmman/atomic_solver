//! Coverage-policy unit tests (plan3 D1): policy orderings, per-class
//! budget resolution, name round-trip. Included by `policy.rs`; also runs
//! via `tests/proofdb.rs`.

use std::collections::HashSet;

use super::*;
use crate::proofdb::fixture::fixture_db;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_pol_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn policies_rank_and_budget_per_class() {
    let dir = tmp_dir("rank");
    let (db, digest) = fixture_db(&dir);
    let f = crate::proofdb::frontier::extract_frontier(&db, &digest, &HashSet::new()).unwrap();

    let p0 = jobs_for_policy(Policy::OpenDeepest, &f);
    assert_eq!(p0.len(), f.c1.len());
    assert!(p0.iter().all(|j| j.class == JobClass::Open));
    assert!(p0.iter().all(|j| j.budget == BUDGET_C1_C3_EVALS));
    assert_eq!(p0[0].path, vec!["f2f3".to_string(), "e7e6".to_string()]);
    assert!(p0.last().unwrap().path.is_empty(), "root last");

    let p1 = jobs_for_policy(Policy::SharpSiblings, &f);
    assert_eq!(p1.len(), f.c2.len() + f.c3.len() + f.c1.len());
    let n2 = f.c2.len();
    // C2 block first, sharpness order (bound asc, ply asc, path asc) — the
    // fixture's win nodes share bound 1 and ply 4, so path-lex ascending.
    assert!(p1[..n2].iter().all(|j| j.class == JobClass::SharpSibling));
    assert!(p1[..n2].iter().all(|j| j.budget == BUDGET_C2_EVALS));
    assert_eq!(p1[0].path[..3], ["f2f3", "e7e6", "g2g4"]);
    for w in p1[..n2].windows(2) {
        let key = |j: &Job| (j.parent_bound, j.ply, j.path.join(" "));
        assert!(key(&w[0]) < key(&w[1]), "C2 sorted");
    }
    // C3 block next (ply ascending, path ascending), then the C1 block.
    let n3 = f.c3.len();
    assert!(
        p1[n2..n2 + n3]
            .iter()
            .all(|j| j.class == JobClass::OpenChild && j.budget == BUDGET_C1_C3_EVALS)
    );
    assert_eq!(p1[n2].path, vec!["a2a3".to_string()], "ply asc, path asc");
    for w in p1[n2..n2 + n3].windows(2) {
        let key = |j: &Job| (j.ply, j.path.join(" "));
        assert!(key(&w[0]) <= key(&w[1]), "C3 sorted");
    }
    assert_eq!(&p1[n2 + n3..], &p0[..], "C1 block = P0's sequence");

    let p2 = jobs_for_policy(Policy::SharpHeavyTail, &f);
    assert_eq!(p2.len(), p1.len(), "same sequence; heavy tier is runtime");
    assert_eq!(p2[0].path, p1[0].path);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn policy_names_round_trip() {
    for (s, p) in [
        ("open-deepest", Policy::OpenDeepest),
        ("sharp-siblings", Policy::SharpSiblings),
        ("sharp-heavy-tail", Policy::SharpHeavyTail),
    ] {
        assert_eq!(p.as_str(), s);
        assert_eq!(Policy::parse(s).unwrap(), p);
    }
    assert!(Policy::parse("deepest").is_err());
}
