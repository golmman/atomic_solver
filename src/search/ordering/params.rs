//! Configurable parameters for `StaticAtomicScorer`.
//!
//! Keeping the tuning knobs in a separate module lets `StaticAtomicScorer`
//! remain focused on scoring logic while the `ScorerParams` struct handles
//! defaults, deserialization, and validation.

use serde::Deserialize;

use atomic_movegen::types::PieceType;

/// Error returned when a `ScorerParams` value violates an invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScorerParamsError {
    Negative {
        field: &'static str,
        value: i32,
    },
    OutOfRange {
        field: &'static str,
        value: i32,
        min: i32,
        max: i32,
    },
    Hierarchy {
        field: &'static str,
        expected_above: &'static str,
        value: i64,
        bound: i64,
    },
    PieceHierarchy {
        value: i64,
        bound: i64,
    },
    Overflow {
        field: &'static str,
        value: i64,
    },
}

impl std::fmt::Display for ScorerParamsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negative { field, value } => {
                write!(f, "{field} must be non-negative, got {value}")
            }
            Self::OutOfRange {
                field,
                value,
                min,
                max,
            } => {
                write!(f, "{field}={value} is out of range [{min}, {max}]")
            }
            Self::Hierarchy {
                field,
                expected_above,
                value,
                bound,
            } => {
                write!(
                    f,
                    "{field} ({value}) must be strictly greater than {expected_above} ({bound})"
                )
            }
            Self::PieceHierarchy { value, bound } => {
                write!(
                    f,
                    "commoner value ({value}) must be strictly greater than the sum of all other pieces ({bound})"
                )
            }
            Self::Overflow { field, value } => {
                write!(f, "{field} computation overflowed i32: {value}")
            }
        }
    }
}

impl std::error::Error for ScorerParamsError {}

/// Piece values used by atomic SEE and promotion bonuses.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PieceValues {
    pub pawn: i32,
    pub knight: i32,
    pub bishop: i32,
    pub rook: i32,
    pub queen: i32,
    pub commoner: i32,
}

impl Default for PieceValues {
    fn default() -> Self {
        Self {
            pawn: 100,
            knight: 320,
            bishop: 330,
            rook: 500,
            queen: 900,
            commoner: 20_000,
        }
    }
}

/// All configurable weights for the static atomic move scorer.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ScorerParams {
    pub score_winning_capture: i32,
    pub score_promotion: i32,
    pub score_capture: i32,
    pub capture_net_scale: i32,
    pub score_threat_last: i32,
    pub score_threat: i32,
    pub score_kamikaze_last: i32,
    pub score_kamikaze: i32,
    pub score_approach: i32,
    pub score_approach_step: i32,
    pub score_center: i32,
    pub score_center_step: i32,
    pub score_pawn_storm: i32,
    pub score_pawn_storm_step: i32,
    pub score_rook_center: i32,
    pub score_rook_open_file: i32,
    pub score_rook_open_file_step: i32,
    pub score_rook_back_rank: i32,
    pub and_pawn_storm_scale: i32,
    pub and_rook_attack_scale: i32,
    pub and_approach_scale: i32,
    pub pieces: PieceValues,
}

impl Default for ScorerParams {
    fn default() -> Self {
        Self {
            score_winning_capture: 100_000_000,
            score_promotion: 1_000_000,
            score_capture: 5_000,
            capture_net_scale: 10,
            score_threat_last: 10_000,
            score_threat: 1_000,
            score_kamikaze_last: 9_000,
            score_kamikaze: 3_000,
            score_approach: 100,
            score_approach_step: 10,
            score_center: 50,
            score_center_step: 10,
            score_pawn_storm: 5_500,
            score_pawn_storm_step: 100,
            score_rook_center: 500,
            score_rook_open_file: 2_000,
            score_rook_open_file_step: 50,
            score_rook_back_rank: 300,
            and_pawn_storm_scale: 50,
            and_rook_attack_scale: 50,
            and_approach_scale: 75,
            pieces: PieceValues::default(),
        }
    }
}

impl ScorerParams {
    /// Look up a piece value, returning 0 for unknown/no piece types.
    #[must_use]
    pub fn piece_value(&self, pt: PieceType) -> i32 {
        match pt {
            PieceType::Pawn => self.pieces.pawn,
            PieceType::Knight => self.pieces.knight,
            PieceType::Bishop => self.pieces.bishop,
            PieceType::Rook => self.pieces.rook,
            PieceType::Queen => self.pieces.queen,
            PieceType::Commoner => self.pieces.commoner,
            _ => 0,
        }
    }

    /// Validate that the parameters preserve the score hierarchy used by the
    /// scorer's early-return categories.
    ///
    /// The checks are intentionally conservative around the three categories
    /// that use early `return`: winning capture, promotion, and general
    /// capture. Lower-tier bonuses (threats, kamikaze, quiet) are allowed to
    /// overlap in configurable ways, so the validation only guards against
    /// accidental overflow and obviously malformed values.
    pub fn validate(&self) -> Result<(), ScorerParamsError> {
        // 1. Non-negativity for all score-related fields.
        let non_negative = [
            ("score_winning_capture", self.score_winning_capture),
            ("score_promotion", self.score_promotion),
            ("score_capture", self.score_capture),
            ("capture_net_scale", self.capture_net_scale),
            ("score_threat_last", self.score_threat_last),
            ("score_threat", self.score_threat),
            ("score_kamikaze_last", self.score_kamikaze_last),
            ("score_kamikaze", self.score_kamikaze),
            ("score_approach", self.score_approach),
            ("score_approach_step", self.score_approach_step),
            ("score_center", self.score_center),
            ("score_center_step", self.score_center_step),
            ("score_pawn_storm", self.score_pawn_storm),
            ("score_pawn_storm_step", self.score_pawn_storm_step),
            ("score_rook_center", self.score_rook_center),
            ("score_rook_open_file", self.score_rook_open_file),
            ("score_rook_open_file_step", self.score_rook_open_file_step),
            ("score_rook_back_rank", self.score_rook_back_rank),
            ("and_pawn_storm_scale", self.and_pawn_storm_scale),
            ("and_rook_attack_scale", self.and_rook_attack_scale),
            ("and_approach_scale", self.and_approach_scale),
            ("pieces.pawn", self.pieces.pawn),
            ("pieces.knight", self.pieces.knight),
            ("pieces.bishop", self.pieces.bishop),
            ("pieces.rook", self.pieces.rook),
            ("pieces.queen", self.pieces.queen),
            ("pieces.commoner", self.pieces.commoner),
        ];
        for (field, value) in non_negative {
            if value < 0 {
                return Err(ScorerParamsError::Negative { field, value });
            }
        }

        // 2. Percent-style scale factors must be in [0, 100].
        for (field, value) in [
            ("and_pawn_storm_scale", self.and_pawn_storm_scale),
            ("and_rook_attack_scale", self.and_rook_attack_scale),
            ("and_approach_scale", self.and_approach_scale),
        ] {
            if !(0..=100).contains(&value) {
                return Err(ScorerParamsError::OutOfRange {
                    field,
                    value,
                    min: 0,
                    max: 100,
                });
            }
        }

        // 3. Piece values: the commoner (king) must dwarf every other piece,
        // because losing it ends the game.
        let other_pieces_sum = i64::from(self.pieces.pawn)
            + i64::from(self.pieces.knight)
            + i64::from(self.pieces.bishop)
            + i64::from(self.pieces.rook)
            + i64::from(self.pieces.queen);
        if i64::from(self.pieces.commoner) <= other_pieces_sum {
            return Err(ScorerParamsError::PieceHierarchy {
                value: i64::from(self.pieces.commoner),
                bound: other_pieces_sum,
            });
        }

        // 4. Hierarchy: winning capture must beat any promotion.
        let max_promotion_score = i64::from(self.score_promotion) + i64::from(self.pieces.queen);
        check_i32("score_promotion + max_piece_value", max_promotion_score)?;
        if i64::from(self.score_winning_capture) <= max_promotion_score {
            return Err(ScorerParamsError::Hierarchy {
                field: "score_winning_capture",
                expected_above: "score_promotion + queen",
                value: i64::from(self.score_winning_capture),
                bound: max_promotion_score,
            });
        }

        // 5. Hierarchy: promotion must beat the highest non-winning capture.
        // The best non-winning capture removes every non-commoner enemy piece
        // with the cheapest attacker (a pawn).
        let max_non_commoner_value = i64::from(self.pieces.queen)
            + i64::from(self.pieces.rook)
            + i64::from(self.pieces.bishop)
            + i64::from(self.pieces.knight);
        let max_capture_net = max_non_commoner_value - i64::from(self.pieces.pawn);
        let max_capture_score =
            i64::from(self.score_capture) + i64::from(self.capture_net_scale) * max_capture_net;
        check_i32("max_capture_score", max_capture_score)?;
        if max_capture_score >= i64::from(self.score_promotion) {
            return Err(ScorerParamsError::Hierarchy {
                field: "score_promotion",
                expected_above: "max_capture_score",
                value: i64::from(self.score_promotion),
                bound: max_capture_score,
            });
        }

        Ok(())
    }
}

fn check_i32(field: &'static str, value: i64) -> Result<(), ScorerParamsError> {
    if value > i64::from(i32::MAX) {
        return Err(ScorerParamsError::Overflow { field, value });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        let params = ScorerParams::default();
        params.validate().expect("default params should validate");
    }

    #[test]
    fn piece_value_uses_configured_table() {
        let params = ScorerParams::default();
        assert_eq!(params.piece_value(PieceType::Pawn), 100);
        assert_eq!(params.piece_value(PieceType::Queen), 900);
        assert_eq!(params.piece_value(PieceType::Commoner), 20_000);
    }

    #[test]
    fn commoner_must_dominate_other_pieces() {
        let params = ScorerParams {
            pieces: PieceValues {
                commoner: 1000,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn negative_score_rejected() {
        let params = ScorerParams {
            score_threat: -1,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn scale_factor_above_percent_rejected() {
        let params = ScorerParams {
            and_pawn_storm_scale: 101,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn scale_factor_negative_rejected() {
        let params = ScorerParams {
            and_pawn_storm_scale: -1,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn scale_factor_in_range_validates() {
        let params = ScorerParams {
            and_pawn_storm_scale: 50,
            ..Default::default()
        };
        assert!(params.validate().is_ok());
    }

    #[test]
    fn promotion_must_beat_max_capture() {
        let params = ScorerParams {
            score_capture: 2_000_000,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn winning_capture_must_beat_promotion() {
        let params = ScorerParams {
            score_winning_capture: 1_000_000,
            ..Default::default()
        };
        assert!(params.validate().is_err());
    }
}
