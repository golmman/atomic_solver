"""Unit tests for §10 weight-file I/O (trainer/weights.py).

Shared with the Rust side together with the sample fixture
``weights.v1.bin`` (renamed to docs/nn_trainer_ref/ there): this file
documents the fixture's expected values (docs/plans/nn/handoff.md).
"""

from __future__ import annotations

import struct

import numpy as np
import pytest

from trainer import weights as W


def tiny_tensors():
    return {
        "W1": np.arange(128 * 768, dtype=np.float32).reshape(128, 768) * 1e-4,
        "b1": np.linspace(-1, 1, 128, dtype=np.float32),
        "W2": np.linspace(-2, 2, 32 * 256, dtype=np.float32).reshape(32, 256),
        "b2": np.arange(32, dtype=np.float32),
        "W3": np.linspace(-1, 1, 4096 * 32, dtype=np.float32).reshape(4096, 32),
        "b3": np.arange(4096, dtype=np.float32) * 0.5,
    }


def test_header_layout(tmp_path):
    p = tmp_path / "w.bin"
    W.write(p, tiny_tensors())
    raw = p.read_bytes()
    assert W.TOTAL_SIZE == 967_312
    assert len(raw) == W.TOTAL_SIZE == 967312
    magic, version, inp, accum, hidden, policy, flags = struct.unpack_from("<IHHHHHH", raw)
    assert magic == 0x4E4E5441
    assert raw[:4] == b"ATNN"  # little-endian magic bytes
    assert (version, inp, accum, hidden, policy, flags) == (1, 768, 128, 32, 4096, 0)


def test_roundtrip_byte_exact(tmp_path):
    """write -> read -> write produces byte-identical files, and the read
    arrays equal the originals bit-for-bit."""
    p1, p2 = tmp_path / "a.bin", tmp_path / "b.bin"
    tensors = tiny_tensors()
    W.write(p1, tensors)
    header, read_back = W.read(p1)
    W.write(p2, read_back)
    assert p1.read_bytes() == p2.read_bytes()
    assert header["version"] == 1 and header["flags"] == 0
    for name, arr in tensors.items():
        assert np.array_equal(read_back[name], arr)
        assert read_back[name].dtype == np.float32


def test_tensor_order_in_file(tmp_path):
    """Tensors appear in §10 order, row-major, right after the 16-byte header."""
    p = tmp_path / "w.bin"
    tensors = tiny_tensors()
    W.write(p, tensors)
    raw = p.read_bytes()[W.HEADER_SIZE :]
    offset = 0
    for name in W.ORDER:
        count = int(np.prod(W.EXPECTED_DIMS[name]))
        chunk = np.frombuffer(raw, dtype="<f4", count=count, offset=offset)
        assert np.array_equal(chunk.reshape(W.EXPECTED_DIMS[name]), tensors[name])
        offset += 4 * count
    assert offset == len(raw)


def test_read_validates_magic_and_version(tmp_path):
    p = tmp_path / "w.bin"
    W.write(p, tiny_tensors())
    raw = bytearray(p.read_bytes())
    raw[0:4] = b"XXXX"
    bad = tmp_path / "bad_magic.bin"
    bad.write_bytes(bytes(raw))
    with pytest.raises(ValueError, match="magic"):
        W.read(bad)
    raw[0:4] = struct.pack("<I", W.MAGIC)
    raw[4:6] = struct.pack("<H", 7)
    bad.write_bytes(bytes(raw))
    with pytest.raises(ValueError, match="version"):
        W.read(bad)


def test_read_validates_size(tmp_path):
    p = tmp_path / "w.bin"
    W.write(p, tiny_tensors())
    raw = p.read_bytes()[:-4]  # truncate one float
    bad = tmp_path / "short.bin"
    bad.write_bytes(raw)
    with pytest.raises(ValueError, match="expected"):
        W.read(bad)


def test_write_validates_shapes(tmp_path):
    p = tmp_path / "w.bin"
    tensors = tiny_tensors()
    tensors["b1"] = np.zeros(127, dtype=np.float32)
    with pytest.raises(ValueError, match="shape"):
        W.write(p, tensors)
    del tensors["b1"]
    with pytest.raises(ValueError, match="missing"):
        W.write(p, tensors)


def test_write_sample_deterministic(tmp_path):
    a, b = tmp_path / "s0a.bin", tmp_path / "s0b.bin"
    c = tmp_path / "s1.bin"
    W.write_sample(a, seed=0)
    W.write_sample(b, seed=0)
    W.write_sample(c, seed=1)
    assert a.read_bytes() == b.read_bytes()
    assert a.read_bytes() != c.read_bytes()
    assert a.stat().st_size == W.TOTAL_SIZE


def test_write_sample_known_entries(tmp_path):
    p = tmp_path / "s.bin"
    W.write_sample(p, seed=0)
    _, tensors = W.read(p)
    assert tensors["W1"][0, 0] == 1.0
    assert tensors["W1"][0, 1] == -0.5
    assert tensors["W1"][127, 767] == 0.25
    assert tensors["b1"][0] == 0.5
    assert tensors["b2"][0] == 0.25
    assert tensors["W3"][0, 0] == 2.0
    assert tensors["b3"][4095] == -0.125
    # Seed-independent regions are all zeros.
    assert int(np.count_nonzero(tensors["W1"])) == 4
    assert int(np.count_nonzero(tensors["W2"])) == 3
    assert int(np.count_nonzero(tensors["W3"])) == 3
    assert int(np.count_nonzero(tensors["b1"])) == 2
    assert int(np.count_nonzero(tensors["b2"])) == 2
    assert int(np.count_nonzero(tensors["b3"])) == 2
