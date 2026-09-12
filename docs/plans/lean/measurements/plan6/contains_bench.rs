// Calibrate `slice::contains` cost for the #17 spike: u64 slices of the mean
// scan length observed in the m22/shuffle-win instrumentation, needle miss
// (the dominant case: the scan almost never hits).
use std::hint::black_box;
use std::time::Instant;

fn bench(len: usize, calls: u64) {
    let v: Vec<u64> = (0..len).map(|i| (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)).collect();
    let needle = 0x1234_5678_9ABC_DEF0u64; // never present (miss path)
    let mut sink = 0u64;
    for _ in 0..1_000_000 {
        if black_box(&v).contains(&needle) {
            sink += 1;
        }
    }
    let t = Instant::now();
    for _ in 0..calls {
        if black_box(&v).contains(&needle) {
            sink += 1;
        }
    }
    let d = t.elapsed();
    let per_call = d.as_nanos() as f64 / calls as f64;
    println!(
        "len={len}: {calls} calls in {:.3}s => {per_call:.2} ns/call, {:.3} ns/element, sink={sink}",
        d.as_secs_f64(),
        per_call / len as f64
    );
}

fn main() {
    bench(9, 15_000_000); // m22 mean scan length ~9.4
    bench(40, 15_000_000); // deep-path regime
    bench(100, 10_000_000); // max-path regime
}
