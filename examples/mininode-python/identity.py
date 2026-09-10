"""Identity: Ed25519 seed file -> keypair -> PeerId.

Docs: docs/protocol/quickstart.md step 1, docs/protocol/wire-format.md section 4.1.
Seed file is bare 32 bytes, mode 0600. PeerId = base58(SHA-256(pubkey)).
"""

import hashlib
import os
from typing import Tuple

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)

SEED_SIZE = 32

_B58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_encode(data: bytes) -> str:
    num = int.from_bytes(data, "big")
    out = []
    while num > 0:
        num, rem = divmod(num, 58)
        out.append(_B58_ALPHABET[rem])
    pad = 0
    for byte in data:
        if byte == 0:
            pad += 1
        else:
            break
    return "1" * pad + "".join(reversed(out))


def base58_decode(text: str) -> bytes:
    num = 0
    for ch in text:
        idx = _B58_ALPHABET.find(ch)
        if idx < 0:
            raise ValueError("invalid base58 character: %r" % ch)
        num = num * 58 + idx
    raw = num.to_bytes((num.bit_length() + 7) // 8, "big")
    pad = 0
    for ch in text:
        if ch == "1":
            pad += 1
        else:
            break
    return b"\x00" * pad + raw


def peer_id_from_pubkey(pubkey: bytes) -> str:
    """PeerId = base58(SHA-256(raw 32-byte ed25519 public key))."""
    if len(pubkey) != SEED_SIZE:
        raise ValueError("pubkey must be 32 bytes, got %d" % len(pubkey))
    return base58_encode(hashlib.sha256(pubkey).digest())


class Identity:
    def __init__(self, seed: bytes):
        if len(seed) != SEED_SIZE:
            raise ValueError("seed must be 32 bytes")
        self.seed = seed
        self.private_key = Ed25519PrivateKey.from_private_bytes(seed)
        self.public_key: Ed25519PublicKey = self.private_key.public_key()
        self.pubkey = self.public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        self.peer_id = peer_id_from_pubkey(self.pubkey)

    def sign(self, message: bytes) -> bytes:
        return self.private_key.sign(message)


def load_or_create_seed(path: str) -> bytes:
    """Load bare 32-byte seed file, or create one with OS CSPRNG (mode 0600)."""
    if os.path.exists(path):
        os.chmod(path, 0o600)  # tighten loose modes
        with open(path, "rb") as fh:
            seed = fh.read()
        if len(seed) != SEED_SIZE:
            raise ValueError(
                "seed file must be exactly 32 bytes, got %d: %s" % (len(seed), path)
            )
        return seed
    seed = os.urandom(SEED_SIZE)
    parent = os.path.dirname(os.path.abspath(path))
    if parent:
        os.makedirs(parent, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        os.write(fd, seed)
    finally:
        os.close(fd)
    return seed


def load_identity(path: str) -> Tuple[Identity, bytes]:
    seed = load_or_create_seed(path)
    return Identity(seed), seed
