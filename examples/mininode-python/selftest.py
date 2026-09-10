"""Offline conformance self-test against docs/protocol/vectors golden bytes.

Run: python3 selftest.py <path-to-docs/protocol/vectors>
Exit 0 on full pass, 1 with a failure report otherwise.
"""

import hashlib
import json
import os
import sys

from identity import Identity, base58_decode, base58_encode, peer_id_from_pubkey
from framing import (
    decode_varint,
    encode_frame,
    encode_varint,
    FrameError,
    VarintOverflow,
)
import pbcodec as pb


def check(name: str, ok: bool, detail: str = "") -> bool:
    print("%s %s %s" % ("PASS" if ok else "FAIL", name, detail))
    return ok


def test_varint(vectors_dir: str) -> bool:
    with open(os.path.join(vectors_dir, "varint.json")) as fh:
        doc = json.load(fh)
    ok = True
    for case in doc["cases"]:
        raw = bytes.fromhex(case["bytes_hex"])
        if case["valid"]:
            value, _ = decode_varint(raw)
            ok &= check(case["name"], value == int(case["value"]), "value=%d" % value)
            ok &= check(case["name"] + "/roundtrip", encode_varint(value) == raw)
        else:
            try:
                decode_varint(raw)
                ok &= check(case["name"], False, "overflow accepted")
            except VarintOverflow:
                ok &= check(case["name"], True)
    return ok


def test_frame(vectors_dir: str) -> bool:
    with open(os.path.join(vectors_dir, "frame.json")) as fh:
        doc = json.load(fh)
    ok = True
    for case in doc["cases"]:
        name = case["name"]
        if name == "frame_hello":
            payload = bytes.fromhex(case["payload_hex"])
            ok &= check(name, encode_frame(payload) == bytes.fromhex(case["frame_hex"]))
        elif name == "frame_empty":
            ok &= check(name, encode_frame(b"") == bytes.fromhex(case["frame_hex"]))
        elif name == "frame_max_size_boundary":
            prefix = encode_varint(case["declared_len"])
            ok &= check(name, prefix == bytes.fromhex(case["frame_prefix_hex"]))
        elif name == "frame_over_limit_rejected":
            try:
                _, _ = decode_varint(bytes.fromhex(case["frame_prefix_hex"]))
                # length decodes fine; rejection happens at frame layer
                from framing import MAX_FRAME_SIZE, FrameTooLarge

                try:
                    encode_frame(b"x" * (MAX_FRAME_SIZE + 1))
                    ok &= check(name, False, "oversize accepted")
                except FrameTooLarge:
                    ok &= check(name, True)
            except FrameError:
                ok &= check(name, True)
    return ok


def test_peer_id(vectors_dir: str) -> bool:
    with open(os.path.join(vectors_dir, "peer-id.json")) as fh:
        doc = json.load(fh)
    ok = True
    for case in doc["cases"]:
        ident = Identity(bytes.fromhex(case["seed_hex"]))
        ok &= check(
            case["name"] + "/pubkey",
            ident.pubkey.hex() == case["public_key_hex"],
        )
        ok &= check(
            case["name"] + "/sha256",
            hashlib.sha256(ident.pubkey).hexdigest() == case["sha256_hex"],
        )
        ok &= check(case["name"] + "/peerid", ident.peer_id == case["peer_id_base58"])
        ok &= check(
            case["name"] + "/b58roundtrip",
            base58_encode(base58_decode(case["peer_id_base58"]))
            == case["peer_id_base58"],
        )
    return ok


def test_rendezvous_register(vectors_dir: str) -> bool:
    with open(os.path.join(vectors_dir, "rendezvous-register.json")) as fh:
        doc = json.load(fh)
    ok = True
    case = doc["cases"][0]
    ident = Identity(bytes.fromhex(case["seed_hex"]))
    ok &= check("register/pubkey", ident.pubkey.hex() == case["public_key_hex"])
    ok &= check(
        "register/peerid",
        ident.peer_id == case["peer_id_base58"],
        ident.peer_id,
    )
    addr = pb.encode_addrmsg(True, "203.0.113.7", 4001)
    ok &= check(
        "register/addrmsg",
        addr.hex() == "0801120b3230332e302e3131332e3718a11f",
        addr.hex(),
    )
    signed = pb.encode_signed_fields(
        case["namespace"],
        base58_decode(ident.peer_id),
        [addr],
        case["ttl_secs"],
        int(case["issued_at"]),
    )
    ok &= check(
        "register/signed_fields",
        signed.hex() == case["signed_fields_hex"],
        signed.hex(),
    )
    ok &= check(
        "register/signature",
        ident.sign(signed).hex() == case["signature_hex"],
    )
    register = pb.encode_register(
        case["namespace"],
        base58_decode(ident.peer_id),
        ident.pubkey,
        [addr],
        case["ttl_secs"],
        ident.sign(signed),
        int(case["issued_at"]),
    )
    ok &= check(
        "register/protobuf",
        register.hex() == case["register_protobuf_hex"],
        register.hex(),
    )
    resp = pb.decode_response(
        b"\x12\x0c" + b"\x0a\x04abcd\x12\x04\x08\x01\x18\x05"
    )
    ok &= check(
        "register/response_decode",
        len(resp.peers) == 1 and resp.peers[0].peer_id == b"abcd" and resp.error == "",
    )
    return ok


def main() -> int:
    vectors_dir = sys.argv[1] if len(sys.argv) > 1 else "../../../docs/protocol/vectors"
    if not os.path.isdir(vectors_dir):
        print("vectors dir not found: %s" % vectors_dir)
        return 1
    ok = True
    ok &= test_varint(vectors_dir)
    ok &= test_frame(vectors_dir)
    ok &= test_peer_id(vectors_dir)
    ok &= test_rendezvous_register(vectors_dir)
    print("SELFTEST-%s" % ("OK" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
