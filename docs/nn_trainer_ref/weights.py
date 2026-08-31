"""docs/spec/nn.md §10 weight-file I/O (byte-level producer contract).

All integers little-endian. One 16-byte header, then the six tensors as
IEEE-754 binary32, little-endian, row-major:

    Header (16 bytes):
      u32 magic        0x4E4E5441   // ASCII "ATNN" as bytes 41 54 4E 4E
      u16 version      1
      u16 input        768
      u16 accumulator  128
      u16 hidden       32
      u16 policy       4096
      u16 flags        0            // 0 = float32, unquantized

    W_1 [128][768], b_1 [128], W_2 [32][256], b_2 [32], W_3 [4096][32], b_3 [4096]

Total size: 16 + 4 * 241,824 = 967,312 bytes. No padding, no reserved field.

Sharing note: this module is the reference reader/writer handed to the
Rust side (Gate 3) — see docs/plans/nn/handoff.md. The normative contract
is docs/spec/nn.md §10; the Rust loader must replicate the validation
hard-errors implemented in :func:`read`.
"""

from __future__ import annotations

import struct

import numpy as np

MAGIC = 0x4E4E5441
VERSION = 1
HEADER = struct.Struct("<IHHHHHH")  # magic, version, input, accum, hidden, policy, flags
HEADER_SIZE = HEADER.size  # 16

ORDER = ("W1", "b1", "W2", "b2", "W3", "b3")

EXPECTED_DIMS = {
    "W1": (128, 768),
    "b1": (128,),
    "W2": (32, 256),
    "b2": (32,),
    "W3": (4096, 32),
    "b3": (4096,),
}

TOTAL_SIZE = HEADER_SIZE + 4 * sum(int(np.prod(sh)) for sh in EXPECTED_DIMS.values())


def _f32(array) -> np.ndarray:
    """float32 little-endian, C-contiguous (row-major)."""
    return np.ascontiguousarray(array, dtype="<f4")


def write(path, tensors: dict[str, np.ndarray]) -> None:
    """Write the six §10 tensors; dims come from the shapes (validated)."""
    missing = [name for name in ORDER if name not in tensors]
    extra = [name for name in tensors if name not in ORDER]
    if missing or extra:
        raise ValueError(f"tensor set mismatch: missing {missing}, extra {extra}")
    dims = []
    for name in ORDER:
        arr = _f32(tensors[name])
        if arr.shape != EXPECTED_DIMS[name]:
            raise ValueError(
                f"{name}: expected shape {EXPECTED_DIMS[name]}, got {arr.shape}"
            )
        dims.append(arr.shape)
    accumulator, input_dim = dims[0]  # W1 [accumulator][input]
    hidden, concat = dims[2]          # W2 [hidden][2*accumulator]
    policy, _ = dims[4]               # W3 [policy][hidden]
    if (
        dims[1] != (accumulator,)
        or dims[3] != (hidden,)
        or dims[5] != (policy,)
        or concat != 2 * accumulator
    ):
        raise ValueError(f"inconsistent dims: {dims}")
    header = HEADER.pack(MAGIC, VERSION, input_dim, accumulator, hidden, policy, 0)
    with open(path, "wb") as f:
        f.write(header)
        for name in ORDER:
            f.write(_f32(tensors[name]).tobytes())


def read(path) -> tuple[dict, dict[str, np.ndarray]]:
    """Read a §10 file: (header dict, {name: numpy float32 array}).

    Validates magic, version, flags, and all four dimension fields against
    the tensor sizes actually present in the file.
    """
    with open(path, "rb") as f:
        data = f.read()
    if len(data) < HEADER_SIZE:
        raise ValueError(f"file too short for header: {len(data)} bytes")
    magic, version, input_dim, accumulator, hidden, policy, flags = HEADER.unpack_from(data)
    if magic != MAGIC:
        raise ValueError(f"bad magic 0x{magic:08X} (expected 0x{MAGIC:08X})")
    if version != VERSION:
        raise ValueError(f"bad version {version} (expected {VERSION})")
    if flags != 0:
        raise ValueError(f"bad flags {flags} (expected 0 = float32, unquantized)")
    shapes = {
        "W1": (accumulator, input_dim),
        "b1": (accumulator,),
        "W2": (hidden, 2 * accumulator),
        "b2": (hidden,),
        "W3": (policy, hidden),
        "b3": (policy,),
    }
    expected = HEADER_SIZE + 4 * sum(int(np.prod(sh)) for sh in shapes.values())
    if len(data) != expected:
        raise ValueError(f"file size {len(data)} != expected {expected} for dims "
                         f"{input_dim}/{accumulator}/{hidden}/{policy}")
    tensors = {}
    offset = HEADER_SIZE
    for name, shape in shapes.items():
        count = int(np.prod(shape))
        arr = np.frombuffer(data, dtype="<f4", count=count, offset=offset)
        tensors[name] = arr.reshape(shape).copy()
        offset += 4 * count
    header = {
        "magic": magic,
        "version": version,
        "input": input_dim,
        "accumulator": accumulator,
        "hidden": hidden,
        "policy": policy,
        "flags": flags,
    }
    return header, tensors


def write_sample(path, seed: int = 0) -> None:
    """Deterministic sample weight file for the Gate-3 Rust loader test.

    All zeros plus a fixed set of known nonzero entries (derived from
    ``seed`` by simple integer arithmetic), so a loader test can assert
    exact bytes and exact values at known offsets. Byte-stable across runs
    and platforms by construction (no RNG, no float formatting).
    """
    tensors = {name: np.zeros(shape, dtype="<f4") for name, shape in EXPECTED_DIMS.items()}
    s = seed & 0xFFFFFFFF
    tensors["W1"][0, 0] = 1.0
    tensors["W1"][0, 1] = -0.5
    tensors["W1"][127, 767] = 0.25
    # Seed-dependent entry, offset to stay clear of the fixed corners above.
    tensors["W1"][1 + s % 126, 1 + s % 766] = 0.125
    tensors["b1"][0] = 0.5
    tensors["b1"][127] = -0.25
    tensors["W2"][0, 0] = 0.5
    tensors["W2"][31, 255] = -0.125
    tensors["W2"][1 + s % 30, 1 + s % 254] = 0.0625
    tensors["b2"][0] = 0.25
    tensors["b2"][31] = -0.5
    tensors["W3"][0, 0] = 2.0
    tensors["W3"][4095, 31] = 1.0
    tensors["W3"][1 + s % 4094, s % 32] = 0.5
    tensors["b3"][0] = 0.125
    tensors["b3"][4095] = -0.125
    write(path, tensors)
