"""Feature extraction for the move-ordering network.

Single source of truth for the docs/spec/nn.md §2 feature contract:

- square index:  ``sq = file + 8 * rank``  (a1 = 0, h8 = 63)
- piece index:   ``p = 6 * color + type``  (color 0 = this view's own side,
  1 = the other side; type 0..5 = pawn..king)
- feature index: ``f = 64 * p + sq``       (f in [0, 768); piece-major,
  square-minor)

NOTE on the spec text: docs/spec/nn.md §2 writes the feature index as
``f = 64 * sq + p``, which cannot fit ``[0, 768)`` (it reaches 4043 for
p=11, sq=63). Since the §2 feature count, the §3 ``W_1`` shape (128 x 768),
and the §10 header field (input = 768) are all pinned to 768, the only
consistent reading keeps the literal ``64`` multiplier on the 12-valued
axis: ``f = 64 * p + sq``. This is also the NNUE/Stockfish convention the
spec references. This module is the pinned Gate-3 contract (decision
recorded in docs/plans/nn/report_external_trainer.md).

Every position is encoded twice, relative to the side to move. The other
view's transform is: swap colors and mirror the board across the center file
(``file -> 7 - file``, rank unchanged).

Standard FEN piece letters only (kings are ``k``/``K``; atomic-movegen 2.1.0
corpora never emit the obsolete ``c``/``C`` commoner spelling). An
en-passant target square in the FEN (e.g. ``w - c6 0 15``) is board syntax,
not a piece letter, and carries no feature (v1 has no en-passant features).
Missing halfmove/fullmove clock fields default to zero.

Sharing note: this module is the reference implementation handed to the
Rust side (Gate 3) — see docs/plans/nn/handoff.md. The normative contract
is docs/spec/nn.md §2; if code and spec ever disagree, the spec (plus the
handoff errata) decides and this module must be brought in line.
"""

from __future__ import annotations

import torch

INPUT_DIM = 768
ACCUMULATOR_DIM = 128
HIDDEN_DIM = 32
POLICY_SIZE = 4096

# FEN piece letter -> (color, type); color 0 = white, 1 = black (FEN colors,
# not view colors). type: 0 pawn, 1 knight, 2 bishop, 3 rook, 4 queen, 5 king.
_PIECE_TYPES = {"p": 0, "n": 1, "b": 2, "r": 3, "q": 4, "k": 5}


def _parse_board(board_field: str) -> list[tuple[int, int, int, int]]:
    """Parse the FEN board field into (color, type, file, rank) tuples.

    ``board_field`` is the first, slash-separated FEN field; ranks are given
    from 8 down to 1.
    """
    pieces: list[tuple[int, int, int, int]] = []
    ranks = board_field.split("/")
    if len(ranks) != 8:
        raise ValueError(f"board field must have 8 ranks, got {len(ranks)}")
    for rank_idx, row in enumerate(ranks):
        # FEN lists ranks 8..1; rank index = 7 - position, rank number = 8 - rank_idx.
        rank = 7 - rank_idx
        file = 0
        for ch in row:
            if ch.isdigit():
                file += int(ch)
            elif ch in ("p", "n", "b", "r", "q", "k", "P", "N", "B", "R", "Q", "K"):
                if file > 7:
                    raise ValueError(f"file overflow in rank row {row!r}")
                color = 0 if ch.isupper() else 1
                pieces.append((color, _PIECE_TYPES[ch.lower()], file, rank))
                file += 1
            else:
                raise ValueError(f"unexpected character {ch!r} in board field")
        if file != 8:
            raise ValueError(f"rank row {row!r} covers {file} of 8 files")
    return pieces


def parse_fen(fen: str) -> tuple[list[tuple[int, int, int, int]], str]:
    """Parse a FEN into (pieces, stm); pieces are (color, type, file, rank).

    Accepts FENs with or without the halfmove/fullmove clock fields (they
    default, and are irrelevant to the features anyway).
    """
    fields = fen.split()
    if len(fields) < 2:
        raise ValueError(f"fen must have at least board and stm fields: {fen!r}")
    board_field, stm = fields[0], fields[1]
    if stm not in ("w", "b"):
        raise ValueError(f"stm must be 'w' or 'b', got {stm!r}")
    return _parse_board(board_field), stm


def _view_indices(
    pieces: list[tuple[int, int, int, int]], own_color: int, mirror: bool
) -> list[int]:
    """Feature indices for one §2 perspective.

    ``own_color`` is the FEN color (0 = white, 1 = black) that counts as the
    view's own side: the side to move for view A, the other side for view B.
    ``mirror`` applies the other-view transform's spatial half
    (``file -> 7 - file``, rank unchanged). The color swap is relative:
    a piece of ``own_color`` gets view color 0, any other piece view 1.
    The side-to-move view uses ``mirror=False`` (the board as-is); the other
    view is the side-to-move view's transform target: swap colors and
    mirror the file.
    """
    indices = []
    for color, ptype, file, rank in pieces:
        view_color = 0 if color == own_color else 1
        view_file = 7 - file if mirror else file
        sq = view_file + 8 * rank
        p = 6 * view_color + ptype
        indices.append(64 * p + sq)
    return indices


def feature_indices(fen: str, stm: str) -> tuple[list[int], list[int]]:
    """Raw §2 feature indices: (stm-view indices, other-view indices)."""
    pieces, fen_stm = parse_fen(fen)
    if stm != fen_stm:
        raise ValueError(f"stm {stm!r} does not match fen stm {fen_stm!r}")
    own = 0 if stm == "w" else 1
    return (
        _view_indices(pieces, own, mirror=False),
        _view_indices(pieces, 1 - own, mirror=True),
    )


def features_for(fen: str, stm: str) -> tuple[torch.Tensor, torch.Tensor]:
    """Two 768-dim binary float32 tensors: (STM view, other view).

    The other view per docs/spec/nn.md §2: colors swapped relative to the
    side to move and the board mirrored across the center file.
    """
    stm_idx, other_idx = feature_indices(fen, stm)
    x_stm = torch.zeros(INPUT_DIM, dtype=torch.float32)
    x_other = torch.zeros(INPUT_DIM, dtype=torch.float32)
    if stm_idx:
        x_stm[torch.tensor(stm_idx, dtype=torch.long)] = 1.0
    if other_idx:
        x_other[torch.tensor(other_idx, dtype=torch.long)] = 1.0
    return x_stm, x_other


def features_batch(fens: list[str], stms: list[str]) -> tuple[torch.Tensor, torch.Tensor]:
    """Stacked features for a list of positions: (N, 768) each view."""
    n = len(fens)
    x_stm = torch.zeros(n, INPUT_DIM, dtype=torch.float32)
    x_other = torch.zeros(n, INPUT_DIM, dtype=torch.float32)
    for i, (fen, stm) in enumerate(zip(fens, stms)):
        stm_idx, other_idx = feature_indices(fen, stm)
        if stm_idx:
            x_stm[i, torch.tensor(stm_idx, dtype=torch.long)] = 1.0
        if other_idx:
            x_other[i, torch.tensor(other_idx, dtype=torch.long)] = 1.0
    return x_stm, x_other


def policy_index(uci: str) -> int:
    """docs/spec/nn.md §5 output index: ``from_sq * 64 + to_sq``.

    Squares use the §2 indexing (``sq = file + 8 * rank``); all four
    promotion variants collapse onto the same (from, to) index.
    """
    if len(uci) < 4:
        raise ValueError(f"bad uci move {uci!r}")
    f_file, f_rank = ord(uci[0]) - ord("a"), ord(uci[1]) - ord("1")
    t_file, t_rank = ord(uci[2]) - ord("a"), ord(uci[3]) - ord("1")
    for v in (f_file, f_rank, t_file, t_rank):
        if not 0 <= v < 8:
            raise ValueError(f"bad uci move {uci!r}")
    from_sq = f_file + 8 * f_rank
    to_sq = t_file + 8 * t_rank
    return from_sq * 64 + to_sq
