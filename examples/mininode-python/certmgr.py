"""Self-signed identity certificate for the QUIC/TLS1.3 path.

Docs: docs/protocol/wire-format.md section 4.2.
- Each side presents a self-signed certificate (mutual auth).
- Certificate carries private extension OID 1.3.6.1.4.1.59015.1 holding the
  raw 32-byte ed25519 public key.
- Verification: only certs with this extension are accepted (no CA lookup),
  and the SPKI public key must equal the extension content byte-for-byte.
- Remote PeerId = base58(SHA-256(extension pubkey)).
"""

import datetime
from typing import Optional

from cryptography import x509
from cryptography.hazmat.primitives import serialization
from cryptography.x509.oid import ObjectIdentifier, ExtendedKeyUsageOID

from identity import Identity

IDENTITY_OID = "1.3.6.1.4.1.59015.1"
CERT_VALID_DAYS = 3650


class IdentityCertError(Exception):
    pass


def _oid() -> ObjectIdentifier:
    return ObjectIdentifier(IDENTITY_OID)


def build_identity_certificate(ident: Identity) -> x509.Certificate:
    """Self-signed cert: identity ed25519 key signs, SPKI = identity pubkey,
    private extension carries the same raw pubkey."""
    now = datetime.datetime.now(datetime.timezone.utc)
    name = x509.Name([x509.NameAttribute(x509.oid.NameOID.COMMON_NAME, ident.peer_id)])
    return (
        x509.CertificateBuilder()
        .subject_name(name)
        .issuer_name(name)
        .public_key(ident.public_key)
        .serial_number(x509.random_serial_number())
        .not_valid_before(now - datetime.timedelta(days=1))
        .not_valid_after(now + datetime.timedelta(days=CERT_VALID_DAYS))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(
            x509.ExtendedKeyUsage([ExtendedKeyUsageOID.CLIENT_AUTH, ExtendedKeyUsageOID.SERVER_AUTH]),
            critical=False,
        )
        .add_extension(x509.UnrecognizedExtension(_oid(), ident.pubkey), critical=False)
        .sign(ident.private_key, None)
    )


def extract_identity_pubkey(cert: x509.Certificate) -> bytes:
    """Return extension pubkey, requiring byte equality with the SPKI key."""
    ext = _find_extension(cert)
    if ext is None:
        raise IdentityCertError("certificate lacks identity extension %s" % IDENTITY_OID)
    if not isinstance(ext.value, x509.UnrecognizedExtension):
        raise IdentityCertError("identity extension has unexpected encoding")
    ext_pubkey = ext.value.value
    spki_pubkey = cert.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    if len(ext_pubkey) != 32:
        raise IdentityCertError("identity extension must carry 32-byte pubkey")
    if ext_pubkey != spki_pubkey:
        raise IdentityCertError("SPKI pubkey differs from identity extension content")
    return ext_pubkey


def _find_extension(cert: x509.Certificate) -> Optional[x509.Extension]:
    try:
        return cert.extensions.get_extension_for_oid(_oid())
    except x509.ExtensionNotFound:
        return None


def verify_identity_certificate(cert: x509.Certificate) -> str:
    """Full third-party check: self-signed in validity, identity anchored.
    Returns the derived PeerId; raises IdentityCertError otherwise."""
    now = datetime.datetime.now(datetime.timezone.utc)
    if now < cert.not_valid_before_utc or now > cert.not_valid_after_utc:
        raise IdentityCertError("certificate outside validity window")
    pubkey = extract_identity_pubkey(cert)
    if cert.issuer != cert.subject:
        raise IdentityCertError("certificate is not self-signed (issuer != subject)")
    cert.public_key().verify(cert.signature, cert.tbs_certificate_bytes)
    from identity import peer_id_from_pubkey

    return peer_id_from_pubkey(pubkey)
