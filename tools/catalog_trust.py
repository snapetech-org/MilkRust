#!/usr/bin/env python3
"""Create a release root from custody-owned PUBLIC Ed25519 keys only.

This command does not generate private keys, handle PEM private keys, change
GitHub settings or publish. Commit its output after independent release review.
"""
from __future__ import annotations
import argparse
import base64
import os
from pathlib import Path
import subprocess

from signed_catalog import KEY_ID, SPKI_PREFIX, encode, openssl, read, require


def public_key(path):
    data = read(path, 4096)
    require(b"PRIVATE" not in data, "only public keys may be used for root configuration")
    if data.startswith(b"-----BEGIN PUBLIC KEY-----"):
        # Work on the bounded snapshot, not a path that could change after the
        # private-key check. OpenSSL receives only a public-key input file.
        import tempfile
        with tempfile.TemporaryDirectory(prefix="milkrust-public-") as folder:
            snapshot = Path(folder) / "public.pem"
            snapshot.write_bytes(data)
            data = openssl("pkey", "-pubin", "-in", snapshot, "-pubout", "-outform", "DER")
    require(len(data) == len(SPKI_PREFIX) + 32 and data.startswith(SPKI_PREFIX),
            "expected Ed25519 SubjectPublicKeyInfo in DER or PEM format")
    return base64.b64encode(data[len(SPKI_PREFIX):]).decode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-key", action="append", required=True, metavar="KEY_ID=PUBLIC_KEY_FILE")
    parser.add_argument("--threshold", type=int, required=True)
    parser.add_argument("--minimum-sequence", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    keys = {}
    require(1 <= len(args.public_key) <= 16, "expected 1 to 16 independent public keys")
    for argument in args.public_key:
        key_id, path = argument.split("=", 1)
        require(KEY_ID.fullmatch(key_id) and key_id not in keys, "invalid or repeated key ID")
        keys[key_id] = public_key(Path(path))
    require(len(set(keys.values())) == len(keys), "quorum aliases are not independent keys")
    require(1 <= args.threshold <= len(keys), "invalid signing quorum")
    require(1 <= args.minimum_sequence <= 9007199254740991, "invalid release sequence floor")
    data = encode({"schemaVersion": 1, "status": "active", "keys": keys,
                   "threshold": args.threshold, "minimumSequence": args.minimum_sequence})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_name(args.output.name + ".partial")
    with temporary.open("xb") as target:
        target.write(data); target.flush(); os.fsync(target.fileno())
    os.replace(temporary, args.output)
    print("Public catalog root written; review, commit and pin it in the app before publication.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, TypeError, OSError, subprocess.SubprocessError):
        raise SystemExit("Public root configuration rejected; no signing or publication was performed.") from None
