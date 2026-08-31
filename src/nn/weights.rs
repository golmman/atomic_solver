//! Weight-file loader for the move-ordering network (`nn.md` §10).
//!
//! Byte-level consumer contract: one 16-byte little-endian header
//! (`u32` magic `0x4E4E5441` = ASCII "ATNN", then six `u16` fields:
//! version, input, accumulator, hidden, policy, flags), followed by six
//! float32 little-endian row-major tensors in the order `W_1 [128][768]`,
//! `b_1 [128]`, `W_2 [32][256]`, `b_2 [32]`, `W_3 [4096][32]`,
//! `b_3 [4096]` — 967,312 bytes total, no padding.
//!
//! Loading hard-errors on wrong magic, version, any dimension, `flags != 0`,
//! or a file size that disagrees with the header dims. Every other piece of
//! the inference contract (feature layout, ClippedReLU max = 1.0) is pinned
//! by the spec, not by the file.
//!
//! This file is larger than 10 KiB because the byte-level parser, the
//! validation error type, the transposing `W_1` load, the tensor accessors,
//! and the fixture-driven conformance tests (header, known entries, every
//! rejection path) are kept together to mirror the reference reader they
//! replicate (`docs/nn_trainer_ref/weights.py`).

use std::path::Path;

use super::{ACCUMULATOR_DIM, HIDDEN_DIM, INPUT_DIM, POLICY_SIZE};

/// File magic: `0x4E4E5441`, little-endian bytes `41 54 4E 4E` = "ATNN".
pub const MAGIC: u32 = 0x4E4E_5441;
/// Weight-file format version (pinned to 1; changes require a version bump).
pub const VERSION: u16 = 1;
/// Header size in bytes: one `u32` plus six `u16`, no padding.
pub const HEADER_SIZE: usize = 16;
/// Total file size implied by the pinned architecture.
pub const TOTAL_SIZE: usize = HEADER_SIZE
    + 4 * (INPUT_DIM * ACCUMULATOR_DIM
        + ACCUMULATOR_DIM
        + HIDDEN_DIM * 2 * ACCUMULATOR_DIM
        + HIDDEN_DIM
        + POLICY_SIZE * HIDDEN_DIM
        + POLICY_SIZE);

/// Everything that can go wrong while loading a §10 weight file.
#[derive(Debug)]
pub enum WeightsError {
    Io(std::io::Error),
    /// Fewer than [`HEADER_SIZE`] bytes.
    TooShortForHeader(usize),
    BadMagic {
        got: u32,
    },
    BadVersion {
        got: u16,
    },
    BadDimension {
        field: &'static str,
        got: u16,
    },
    BadFlags {
        got: u16,
    },
    SizeMismatch {
        got: usize,
        expected: usize,
    },
}

impl std::fmt::Display for WeightsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read weight file: {e}"),
            Self::TooShortForHeader(got) => {
                write!(
                    f,
                    "file too short for header: {got} bytes (need {HEADER_SIZE})"
                )
            }
            Self::BadMagic { got } => {
                write!(f, "bad magic 0x{got:08X} (expected 0x{MAGIC:08X})")
            }
            Self::BadVersion { got } => write!(f, "bad version {got} (expected {VERSION})"),
            Self::BadDimension { field, got } => write!(
                f,
                "bad {field} dimension {got} (expected {})",
                expected_dim(field)
            ),
            Self::BadFlags { got } => {
                write!(f, "bad flags {got} (expected 0 = float32, unquantized)")
            }
            Self::SizeMismatch { got, expected } => {
                write!(
                    f,
                    "file size {got} != expected {expected} for the header dims"
                )
            }
        }
    }
}

impl std::error::Error for WeightsError {}

impl From<std::io::Error> for WeightsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn expected_dim(field: &str) -> u16 {
    match field {
        "input" => INPUT_DIM as u16,
        "accumulator" => ACCUMULATOR_DIM as u16,
        "hidden" => HIDDEN_DIM as u16,
        "policy" => POLICY_SIZE as u16,
        _ => unreachable!("unknown header field {field}"),
    }
}

/// Parsed §10 tensors.
///
/// `W_1` is kept transposed in memory (`[input][accumulator]`) so that the
/// column `W_1[:, f]` — the §4 incremental-update vector for feature `f` —
/// is one contiguous 128-float slice; the file itself stays row-major
/// `[128][768]` and is transposed once at load.
#[derive(Debug, Clone)]
pub struct NnWeights {
    w1_cols: Vec<f32>,
    b1: [f32; ACCUMULATOR_DIM],
    w2: Vec<f32>,
    b2: [f32; HIDDEN_DIM],
    w3: Vec<f32>,
    b3: Vec<f32>,
}

impl NnWeights {
    /// Load and validate a §10 weight file from `path`.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, WeightsError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Validate a §10 byte image and parse the six tensors.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WeightsError> {
        if bytes.len() < HEADER_SIZE {
            return Err(WeightsError::TooShortForHeader(bytes.len()));
        }
        let read_u16 = |off: usize| u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != MAGIC {
            return Err(WeightsError::BadMagic { got: magic });
        }
        let version = read_u16(4);
        if version != VERSION {
            return Err(WeightsError::BadVersion { got: version });
        }
        for (field, off) in [
            ("input", 6),
            ("accumulator", 8),
            ("hidden", 10),
            ("policy", 12),
        ] {
            let got = read_u16(off);
            if got != expected_dim(field) {
                return Err(WeightsError::BadDimension { field, got });
            }
        }
        let flags = read_u16(14);
        if flags != 0 {
            return Err(WeightsError::BadFlags { got: flags });
        }
        if bytes.len() != TOTAL_SIZE {
            return Err(WeightsError::SizeMismatch {
                got: bytes.len(),
                expected: TOTAL_SIZE,
            });
        }

        let mut off = HEADER_SIZE;
        let mut read_tensor = |count: usize| {
            let mut out = Vec::with_capacity(count);
            out.extend((0..count).map(|i| {
                let at = off + 4 * i;
                f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
            }));
            off += 4 * count;
            out
        };

        // W_1, row-major [accumulator][input] in the file; transposed into
        // [input][accumulator] so each feature column is contiguous.
        let mut w1_cols = vec![0.0f32; INPUT_DIM * ACCUMULATOR_DIM];
        for r in 0..ACCUMULATOR_DIM {
            for c in 0..INPUT_DIM {
                w1_cols[c * ACCUMULATOR_DIM + r] = read_tensor(1)[0];
            }
        }
        let b1: [f32; ACCUMULATOR_DIM] = read_tensor(ACCUMULATOR_DIM)
            .try_into()
            .expect("b_1 has exactly ACCUMULATOR_DIM entries");
        let w2 = read_tensor(HIDDEN_DIM * 2 * ACCUMULATOR_DIM);
        let b2: [f32; HIDDEN_DIM] = read_tensor(HIDDEN_DIM)
            .try_into()
            .expect("b_2 has exactly HIDDEN_DIM entries");
        let w3 = read_tensor(POLICY_SIZE * HIDDEN_DIM);
        let b3 = read_tensor(POLICY_SIZE);
        debug_assert_eq!(off, TOTAL_SIZE);

        Ok(Self {
            w1_cols,
            b1,
            w2,
            b2,
            w3,
            b3,
        })
    }

    /// The §4 incremental-update column `W_1[:, f]` for feature `f`
    /// (128 contiguous floats).
    #[must_use]
    pub fn w1_column(&self, feature: usize) -> &[f32] {
        &self.w1_cols[feature * ACCUMULATOR_DIM..(feature + 1) * ACCUMULATOR_DIM]
    }

    /// Row-major `W_1[row][col]` accessor (tests / diagnostics).
    #[must_use]
    pub fn w1_at(&self, row: usize, col: usize) -> f32 {
        self.w1_cols[col * ACCUMULATOR_DIM + row]
    }

    /// The stage-1 bias `b_1`.
    #[must_use]
    pub fn b1(&self) -> &[f32; ACCUMULATOR_DIM] {
        &self.b1
    }

    /// Row `r` of `W_2` (256 floats, concat order: stm half first).
    #[must_use]
    pub fn w2_row(&self, r: usize) -> &[f32] {
        let width = 2 * ACCUMULATOR_DIM;
        &self.w2[r * width..(r + 1) * width]
    }

    /// The hidden-layer bias `b_2`.
    #[must_use]
    pub fn b2(&self) -> &[f32; HIDDEN_DIM] {
        &self.b2
    }

    /// Row `policy_index` of `W_3` (32 floats, dot with `h`).
    #[must_use]
    pub fn w3_row(&self, policy_index: usize) -> &[f32] {
        &self.w3[policy_index * HIDDEN_DIM..(policy_index + 1) * HIDDEN_DIM]
    }

    /// The output bias `b_3[idx]`.
    #[must_use]
    pub fn b3_at(&self, policy_index: usize) -> f32 {
        self.b3[policy_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/nn_trainer_ref/fixtures/weights.v1.bin")
    }

    fn load_fixture() -> NnWeights {
        NnWeights::from_path(fixture()).expect("fixture weight file must load")
    }

    /// Mutate the fixture bytes at `off` and try to load the result.
    fn mutated(off: usize, value: u8) -> Vec<u8> {
        let mut bytes = std::fs::read(fixture()).expect("fixture must be readable");
        bytes[off] = value;
        bytes
    }

    #[test]
    fn total_size_matches_spec() {
        assert_eq!(TOTAL_SIZE, 967_312);
        assert_eq!(HEADER_SIZE, 16);
        let bytes = std::fs::read(fixture()).expect("fixture must be readable");
        assert_eq!(bytes.len(), TOTAL_SIZE);
        assert_eq!(
            *bytes.first_chunk::<4>().unwrap(),
            MAGIC.to_le_bytes(),
            "fixture must start with the ATNN magic"
        );
    }

    #[test]
    fn fixture_header_and_corner_entries() {
        let w = load_fixture();
        assert_eq!(*w.b1(), {
            let mut b = [0.0f32; ACCUMULATOR_DIM];
            b[0] = 0.5;
            b[127] = -0.25;
            b
        });
        assert_eq!(w.w1_at(0, 0), 1.0);
        assert_eq!(w.w1_at(0, 1), -0.5);
        assert_eq!(w.w1_at(127, 767), 0.25);
        // Seed-0 derived entry: W_1[1 + 0 % 126][1 + 0 % 766] = 0.125.
        assert_eq!(w.w1_at(1, 1), 0.125);
        assert_eq!(w.b2()[0], 0.25);
        assert_eq!(w.b2()[31], -0.5);
        assert_eq!(w.w2_row(0)[0], 0.5);
        assert_eq!(w.w2_row(31)[255], -0.125);
        // Seed-0 derived entry: W_2[1 + 0 % 30][1 + 0 % 254] = 0.0625.
        assert_eq!(w.w2_row(1)[1], 0.0625);
        assert_eq!(w.w3_row(0)[0], 2.0);
        assert_eq!(w.w3_row(4095)[31], 1.0);
        // Seed-0 derived entry: W_3[1 + 0 % 4094][0 % 32] = 0.5.
        assert_eq!(w.w3_row(1)[0], 0.5);
        assert_eq!(w.b3_at(0), 0.125);
        assert_eq!(w.b3_at(4095), -0.125);
    }

    #[test]
    fn fixture_nonzero_counts_match_reference() {
        // Exact counts per docs/nn_trainer_ref/test_weights.py: 13 fixed
        // corner entries plus one seed-0 entry per weight tensor.
        let w = load_fixture();
        let count = |xs: &[f32]| xs.iter().filter(|x| **x != 0.0).count();
        assert_eq!(count(&w.w1_cols), 4);
        assert_eq!(count(w.b1()), 2);
        assert_eq!(count(&w.w2), 3);
        assert_eq!(count(w.b2()), 2);
        assert_eq!(count(&w.w3), 3);
        assert_eq!(count(&w.b3), 2);
        // All other bias entries are zero.
        assert!(w.b1()[1..127].iter().all(|&x| x == 0.0));
        assert_eq!(w.b3_at(1), 0.0);
    }

    #[test]
    fn w1_transpose_round_trips() {
        let w = load_fixture();
        // Column access must agree with the row-major view.
        for (r, x) in w.w1_column(0).iter().enumerate() {
            assert_eq!(*x, w.w1_at(r, 0));
        }
        assert_eq!(w.w1_column(767)[127], w.w1_at(127, 767));
        assert_eq!(w.w1_column(1)[1], w.w1_at(1, 1));
    }

    #[test]
    fn bad_magic_rejected() {
        let err = NnWeights::from_bytes(&mutated(0, 0x58)).unwrap_err();
        assert!(matches!(err, WeightsError::BadMagic { .. }), "{err}");
    }

    #[test]
    fn bad_version_rejected() {
        let err = NnWeights::from_bytes(&mutated(4, 7)).unwrap_err();
        assert!(matches!(err, WeightsError::BadVersion { got: 7 }), "{err}");
    }

    #[test]
    fn bad_dimensions_rejected() {
        for (off, field) in [
            (6, "input"),
            (8, "accumulator"),
            (10, "hidden"),
            (12, "policy"),
        ] {
            let err = NnWeights::from_bytes(&mutated(off, 0x01)).unwrap_err();
            assert!(
                matches!(err, WeightsError::BadDimension { field: f, .. } if f == field),
                "{field}: {err}"
            );
        }
    }

    #[test]
    fn bad_flags_rejected() {
        let err = NnWeights::from_bytes(&mutated(14, 1)).unwrap_err();
        assert!(matches!(err, WeightsError::BadFlags { got: 1 }), "{err}");
    }

    #[test]
    fn truncated_file_rejected() {
        let bytes = std::fs::read(fixture()).unwrap();
        let err = NnWeights::from_bytes(&bytes[..bytes.len() - 4]).unwrap_err();
        assert!(
            matches!(
                err,
                WeightsError::SizeMismatch {
                    expected: 967_312,
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn short_header_rejected() {
        let err = NnWeights::from_bytes(&[0u8; 8]).unwrap_err();
        assert!(matches!(err, WeightsError::TooShortForHeader(8)), "{err}");
    }

    #[test]
    fn missing_file_is_io_error() {
        let result = NnWeights::from_path("/nonexistent/weights.v1.bin");
        assert!(matches!(result, Err(WeightsError::Io(_))));
    }
}
