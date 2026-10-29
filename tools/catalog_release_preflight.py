#!/usr/bin/env python3
"""Keyless, offline preflight for production visualizer-catalog delivery.

This intentionally does not read environment variables, API credentials,
GitHub secrets, or private signing keys. It validates source-controlled
configuration and emits the remaining operator gates without provisioning or
publishing anything.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
KEY_ID = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def check_root(path: Path) -> dict:
    root = load_json(path)
    if not isinstance(root, dict) or root.get("schemaVersion") != 1:
        raise ValueError("trust root must be an object with schemaVersion 1")
    status = root.get("status")
    keys = root.get("keys")
    threshold = root.get("threshold")
    floor = root.get("minimumSequence")
    if not isinstance(keys, dict) or len(keys) > 16:
        raise ValueError("trust root keys must be an object with at most 16 entries")
    if type(threshold) is not int or not 1 <= threshold <= max(1, len(keys)):
        raise ValueError("trust root threshold is invalid")
    if type(floor) is not int or not 1 <= floor <= 9007199254740991:
        raise ValueError("trust root minimumSequence is invalid")
    decoded = []
    for key_id, encoded in keys.items():
        if not isinstance(key_id, str) or not KEY_ID.fullmatch(key_id):
            raise ValueError("trust root contains an invalid key ID")
        if not isinstance(encoded, str):
            raise ValueError("trust root public key values must be base64 strings")
        try:
            raw = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error):
            raise ValueError("trust root contains invalid base64 public-key material") from None
        if len(raw) != 32:
            raise ValueError("trust root Ed25519 public keys must decode to 32 bytes")
        decoded.append(raw)
    if len(set(decoded)) != len(decoded):
        raise ValueError("trust root contains duplicate public keys")
    if status not in ("active", "not-provisioned"):
        raise ValueError("trust root status must be active or not-provisioned")
    if status == "active" and not keys:
        raise ValueError("active trust root has no public keys")
    if status == "not-provisioned" and keys:
        raise ValueError("not-provisioned trust root must not contain keys")
    return {"status": status, "keyCount": len(keys), "threshold": threshold,
            "minimumSequence": floor}


def check_workflow(path: Path) -> dict:
    source = path.read_text(encoding="utf-8")
    required = {
        "manualDispatch": "workflow_dispatch:" in source,
        "mainOnly": "if: github.ref == 'refs/heads/main'" in source,
        "protectedEnvironment": "environment: visualizer-catalog" in source,
        "privateJitGroup": '"group":"ng-shared-main-jit"' in source
            and '"self-hosted"' in source,
        "writePermissionScopedToPublishJob": "permissions:\n      contents: write" in source,
        "checkoutCredentialsDisabled": "persist-credentials: false" in source,
        "secretNameReferencedButNotReadHere": "secrets.MILKRUST_CATALOG_PRIVATE_KEYS" in source,
    }
    if not all(required.values()):
        missing = ", ".join(name for name, passed in required.items() if not passed)
        raise ValueError("publication workflow source guard failed: " + missing)
    if "ubuntu-latest" in source or "macos-latest" in source or "windows-latest" in source:
        raise ValueError("publication workflow must not declare GitHub-hosted runners")
    return required


def build_report(trust_path: Path, workflow_path: Path, catalog_path: Path) -> dict:
    root = check_root(trust_path)
    workflow = check_workflow(workflow_path)
    catalog = load_json(catalog_path)
    if not isinstance(catalog, dict) or catalog.get("schemaVersion") != 1:
        raise ValueError("download catalog must be an object with schemaVersion 1")
    if not isinstance(catalog.get("packs"), list):
        raise ValueError("download catalog packs must be a list")

    blockers = []
    if root["status"] != "active":
        blockers.append("Create and independently review an active public trust root from approved key custody.")
    blockers.extend([
        "Pin the reviewed public trust root and immutable source revision in the PlezyNG release app.",
        "Provision the protected visualizer-catalog environment and signing credentials in approved custody; this preflight does not inspect or configure them.",
        "Confirm live private ephemeral JIT broker routing, runner capacity, environment approval rules and branch protections.",
        "Publish only reviewed immutable archives with complete rights and qualification evidence.",
        "After publication, verify signature, expiry, rollback, revocation and offline behavior in installed apps.",
    ])
    return {
        "schemaVersion": 1,
        "status": "prepared-blocked-on-provisioning" if blockers else "ready-for-release-review",
        "credentialsInspected": False,
        "networkUsed": False,
        "trustRoot": root,
        "publicationWorkflow": workflow,
        "catalogPackCount": len(catalog["packs"]),
        "productionSigningReady": False,
        "blockers": blockers,
        "note": "A successful source preflight is not proof of key custody, GitHub environment settings, runner capacity, publication, or installed-app qualification.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--trust", type=Path, default=ROOT / "content/catalog-trust.json")
    parser.add_argument("--workflow", type=Path,
                        default=ROOT / ".github/workflows/publish-visualizer-catalog.yml")
    parser.add_argument("--catalog", type=Path, default=ROOT / "content/downloads/catalog.json")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = build_report(args.trust, args.workflow, args.catalog)
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as error:
        print(f"catalog preflight failed: {error}", file=sys.stderr)
        raise SystemExit(2) from None
