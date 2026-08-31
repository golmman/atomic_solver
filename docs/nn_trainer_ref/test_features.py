"""Unit tests for the §2 feature extraction (trainer/features.py).

The hand-computed index expectations in this file are shared with the Rust
side as conformance vectors (docs/plans/nn/handoff.md); the Rust feature
extractor must reproduce them exactly.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import torch

from trainer.features import (
    INPUT_DIM,
    POLICY_SIZE,
    feature_indices,
    features_batch,
    features_for,
    parse_fen,
    policy_index,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
REAL_CORPUS = REPO_ROOT / "data" / "corpus" / "train.ndjson"


def indices_of(x: torch.Tensor) -> list[int]:
    return sorted(torch.nonzero(x).flatten().tolist())


def test_lone_white_king_a1_stm_w():
    """The plan §3 worked example (with the pinned f = 64*p + sq layout):
    a lone white king on a1, stm = w.

    View A: sq = 0 + 8*0 = 0, p = 6*0 + 5 = 5, f = 64*5 + 0 = 320.
    View B: mirror file 0 -> 7, sq = 7, color swapped -> p = 6*1 + 5 = 11,
    f = 64*11 + 7 = 711.
    """
    x_stm, x_other = features_for("8/8/8/8/8/8/8/K7 w - - 0 1", "w")
    assert indices_of(x_stm) == [320]
    assert indices_of(x_other) == [711]


def test_spec_fen_all_views():
    """The spec's example position `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1`
    (black king e8, white rook e1, white king g1).

    View A (colors relative to white):
      black king e8: p = 11, sq = 4 + 56 = 60 -> f = 64*11 + 60 = 764
      white rook e1: p = 3,  sq = 4           -> f = 64*3 + 4  = 196
      white king g1: p = 5,  sq = 6           -> f = 64*5 + 6  = 326
    View B (colors swapped, files mirrored):
      black king e8 -> own side,  p = 5,  sq = 3 + 56 = 59 -> f = 379
      white rook e1 -> other side, p = 9, sq = 3           -> f = 579
      white king g1 -> other side, p = 11, sq = 1          -> f = 705
    """
    x_stm, x_other = features_for("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1", "w")
    assert indices_of(x_stm) == [196, 326, 764]
    assert indices_of(x_other) == [379, 579, 705]


def test_other_view_transform_color_and_file():
    """White pawn e2, stm w.

    View A: sq = 4 + 8*1 = 12, p = 6*0 + 0 = 0, f = 64*0 + 12 = 12.
    View B: black pawn, mirror file 4 -> 3, sq = 3 + 8 = 11, p = 6*1 + 0 = 6,
    f = 64*6 + 11 = 395.
    """
    x_stm, x_other = features_for("8/8/8/8/8/8/4P3/8 w - - 0 1", "w")
    assert indices_of(x_stm) == [12]
    assert indices_of(x_other) == [395]


def test_black_king_other_side_view():
    """Black king h8, stm w: it is the other side (color 1) in the STM view.

    View A: sq = 7 + 56 = 63, p = 6*1 + 5 = 11, f = 64*11 + 63 = 767.
    View B: own side, mirror file 7 -> 0, sq = 0 + 56 = 56, p = 5,
    f = 64*5 + 56 = 376.
    """
    x_stm, x_other = features_for("7k/8/8/8/8/8/8/8 w - - 0 1", "w")
    assert indices_of(x_stm) == [767]
    assert indices_of(x_other) == [376]


def test_black_rook_a1_views():
    """Black rook a1, stm w.

    View A: p = 6*1 + 3 = 9, sq = 0, f = 64*9 + 0 = 576.
    View B: white rook, mirror file 0 -> 7, sq = 7, p = 6*0 + 3 = 3,
    f = 64*3 + 7 = 199.
    """
    x_stm, x_other = features_for("8/8/8/8/8/8/8/r7 w - - 0 1", "w")
    assert indices_of(x_stm) == [576]
    assert indices_of(x_other) == [199]


def test_black_stm_own_side_view_not_mirrored():
    """Black king h8, stm b: own side (color 0), and the STM view is the
    board as-is (no mirror).

    View A: sq = 7 + 56 = 63, p = 5, f = 64*5 + 63 = 383.
    View B: white king, mirror file 7 -> 0, sq = 0 + 56 = 56, p = 6*1 + 5
    = 11, f = 64*11 + 56 = 760.
    """
    x_stm, x_other = features_for("7k/8/8/8/8/8/8/8 b - - 0 1", "b")
    assert indices_of(x_stm) == [383]
    assert indices_of(x_other) == [760]


def test_full_position_feature_counts():
    fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    x_stm, x_other = features_for(fen, "w")
    assert x_stm.shape == (INPUT_DIM,) and x_other.shape == (INPUT_DIM,)
    assert x_stm.dtype == torch.float32
    assert int(x_stm.sum()) == 32 == int(x_other.sum())
    assert int((x_stm > 0).sum()) == 32  # binary, one bit per piece
    # A position and its color-swapped mirror share the STM view's index set
    # with the other view's index set of the original only if symmetric;
    # here they must differ (the starting position is rank-asymmetric).
    assert indices_of(x_stm) != indices_of(x_other)


def test_feature_indices_agree_with_tensors():
    fen = "r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30"
    x_stm, x_other = features_for(fen, "w")
    stm_idx, other_idx = feature_indices(fen, "w")
    assert indices_of(x_stm) == sorted(stm_idx)
    assert indices_of(x_other) == sorted(other_idx)
    assert len(stm_idx) == 19  # pieces on the board
    assert max(stm_idx + other_idx) < INPUT_DIM


def test_parse_fen_defaults_and_ep_field():
    pieces, stm = parse_fen("8/8/8/8/8/8/8/8 w - -")
    assert pieces == [] and stm == "w"
    # 'c6' here is the en-passant target square, not a commoner piece letter.
    pieces, stm = parse_fen("8/8/2P5/8/8/8/8/8 b - c6 0 15")
    assert stm == "b"
    assert len(pieces) == 1
    color, ptype, pfile, prank = pieces[0]
    assert (color, ptype, pfile, prank) == (0, 0, 2, 5)  # white pawn c6


def test_parse_fen_rejects_garbage():
    with pytest.raises(ValueError):
        parse_fen("notafen")
    with pytest.raises(ValueError):
        parse_fen("8/8/8/8/8/8/8/8 x - - 0 1")  # bad stm
    with pytest.raises(ValueError):
        parse_fen("8/8/8/8/8/8/8 w - - 0 1")  # 7 ranks
    with pytest.raises(ValueError):
        parse_fen("8/8/8/8/8/8/8/9 w - - 0 1")  # rank covers 9 files
    with pytest.raises(ValueError):
        parse_fen("8/8/8/8/8/8/8/z7 w - - 0 1")  # bad piece letter
    with pytest.raises(ValueError):
        feature_indices("8/8/8/8/8/8/8/8 w - - 0 1", "b")  # stm mismatch


def test_policy_index_layout():
    """§5: policy_index = from_sq * 64 + to_sq with sq = file + 8*rank."""
    assert policy_index("a1a2") == 0 * 64 + 8
    assert policy_index("e2e4") == (4 + 8) * 64 + (4 + 24)
    assert policy_index("h8h1") == 63 * 64 + 7
    # Promotions collapse onto the same (from, to) index.
    assert policy_index("a7a8q") == policy_index("a7a8n") == policy_index("a7a8")
    with pytest.raises(ValueError):
        policy_index("e2")
    with pytest.raises(ValueError):
        policy_index("z2e4")


def test_policy_size_bounds():
    assert policy_index("h8h8") < POLICY_SIZE
    assert policy_index("a1a1") >= 0


# ------------------------------------------------------------ real corpus


@pytest.mark.slow
@pytest.mark.skipif(not REAL_CORPUS.exists(), reason="real corpus not present")
def test_real_corpus_features():
    """Feature extraction over the kept rows of the real corpus: binary
    768-dim tensors, one active feature per piece, all indices in range."""
    from trainer import corpus as corpus_mod

    data = corpus_mod.load_corpus(REAL_CORPUS)
    kept = [r for r in data.rows if not r.partial]
    x_stm, x_other = features_batch([r.fen for r in kept], [r.stm for r in kept])
    assert x_stm.shape == (len(kept), INPUT_DIM)
    assert x_other.shape == (len(kept), INPUT_DIM)
    assert bool(((x_stm == 0) | (x_stm == 1)).all())
    assert bool(((x_other == 0) | (x_other == 1)).all())
    assert int(x_stm.max()) == 1 and int(x_other.max()) == 1
    # Every row's active-feature count equals its piece count in both views.
    counts = x_stm.sum(dim=1)
    for i in range(0, len(kept), 997):  # deterministic sample
        pieces = len(kept[i].fen.split()[0].replace("/", "")) - sum(
            c.isdigit() for c in kept[i].fen.split()[0]
        )
        assert int(counts[i]) == pieces
        assert int(x_other[i].sum()) == pieces
