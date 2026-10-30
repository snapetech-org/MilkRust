#!/usr/bin/env python3
"""Prepare, sign, verify and publish MilkRust catalog v1 on a private JIT runner.

OpenSSL 3 supplies Ed25519. Private keys are input files or environment secrets;
only payloads, signatures and public key IDs are printed/written to artifacts.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import os
import re
import stat
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from validate_pack_dependencies import validate_pack_dependencies
from datetime import datetime, timedelta, timezone
from pathlib import Path

from catalog_qualification import FIXTURE, TARGETS, qualify

DOMAIN = b"MilkRust.catalog.v1\n"
MAX_PAYLOAD = 1024 * 1024
MAX_ENVELOPE = 2 * MAX_PAYLOAD
MAX_ARCHIVE = 32 * MAX_PAYLOAD
MAX_EXPANDED = 64 * MAX_PAYLOAD
SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,79}\Z")
KEY_ID = re.compile(r"[a-z0-9_-]{1,64}\Z")
ARCHIVE_URL = re.compile(r"https://raw\.githubusercontent\.com/snapetech-org/MilkRust/([a-f0-9]{40})/content/downloads/([A-Za-z0-9._-]+\.rmpack)\Z")
LICENSES = {
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "CC-BY-3.0", "CC-BY-4.0",
    "CC-BY-SA-3.0", "CC-BY-SA-4.0", "CC0-1.0", "GPL-2.0-only", "GPL-2.0-or-later",
    "GPL-3.0-only", "GPL-3.0-or-later", "ISC", "LGPL-2.1-only", "LGPL-2.1-or-later",
    "LGPL-3.0-only", "LGPL-3.0-or-later", "MIT", "Unlicense", "Zlib",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path, limit):
    with Path(path).open("rb") as source:
        data = source.read(limit + 1)
    require(len(data) <= limit, "input exceeds limit")
    return data


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def decode(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(ValueError("non-finite JSON")))


def identifier(value):
    require(isinstance(value, str) and IDENTIFIER.fullmatch(value) and not value.endswith((".", ".previous", ".partial"))
            and not re.match(r"^(con|prn|aux|nul|com[0-9]|lpt[0-9])($|\.)", value, re.I), "invalid portable pack identity")


def validate_payload(payload, now, allow_expired=False):
    require(len(payload) <= MAX_PAYLOAD, "payload too large")
    data = decode(payload)
    require(isinstance(data, dict) and data.get("schemaVersion") == 1, "unsupported catalog schema")
    require(type(data.get("sequence")) is int and 0 < data["sequence"] <= 9007199254740991, "invalid sequence")
    def date(key):
        value = data.get(key)
        require(isinstance(value, str) and value.endswith("Z"), "catalog dates must be UTC")
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    issued, expires = date("issuedAt"), date("expiresAt")
    require(issued <= now + timedelta(minutes=5) and issued < expires <= issued + timedelta(days=31), "invalid validity window")
    require(allow_expired or expires > now, "catalog expired")
    require(isinstance(data.get("entries"), list) and len(data["entries"]) <= 1000, "invalid entries")
    ids = set()
    for entry in data["entries"]:
        require(isinstance(entry, dict), "invalid entry")
        identifier(entry.get("id")); identifier(entry.get("version"))
        require(entry["id"] not in ids, "more than one current version of a pack")
        ids.add(entry["id"])
        require(entry.get("distributionStatus") == "redistributable" and entry.get("license") in LICENSES, "uncleared pack")
        for key, limit in {"name": 200, "description": 4096, "contentNotice": 8192, "sourceUrl": 2048}.items():
            value = entry.get(key)
            require(isinstance(value, str) and value.strip() and len(value) <= limit and "\0" not in value, "invalid entry text")
        require(isinstance(entry.get("searchText", ""), str) and len(entry.get("searchText", "")) <= 4096, "invalid search text")
        source = urllib.parse.urlsplit(entry["sourceUrl"])
        require(source.scheme == "https" and source.hostname and not source.username and not source.password, "invalid source URL")
        require(ARCHIVE_URL.fullmatch(entry.get("archiveUrl", "")), "archives must be immutable MilkRust content")
        require(re.fullmatch(r"[a-f0-9]{64}", entry.get("sha256", "")), "invalid archive digest")
        kind = entry.get("kind", "presets")
        require(kind in ("presets", "textures"), "invalid pack kind")
        for key, limit in {"archiveBytes": MAX_ARCHIVE, "expandedBytes": MAX_EXPANDED, "presetCount": 2500}.items():
            minimum = 0 if key == "presetCount" and kind == "textures" else 1
            require(type(entry.get(key)) is int and minimum <= entry[key] <= limit, "invalid entry size")
        require(kind != "textures" or entry["presetCount"] == 0, "texture pack cannot contain presets")
        qualification = entry.get("qualification")
        require(isinstance(qualification, dict) and isinstance(qualification.get("targets"), list), "missing qualification metadata")
        if qualification.get("status") == "sample-fixture-only":
            require((entry["id"], entry["version"], entry["sha256"]) == FIXTURE
                    and qualification["targets"] == []
                    and "reportUrl" not in qualification and "reportSha256" not in qualification,
                    "fixture qualification exception does not apply")
        else:
            require(qualification.get("status") == "qualified"
                    and len(qualification["targets"]) == len(TARGETS)
                    and all(isinstance(t, str) for t in qualification["targets"])
                    and set(qualification["targets"]) == TARGETS, "incomplete qualification coverage")
            require(re.fullmatch(r"https://raw\.githubusercontent\.com/snapetech-org/MilkRust/[a-f0-9]{40}/content/qualification/reports/[A-Za-z0-9_-]+\.json",
                                 qualification.get("reportUrl", ""))
                    and re.fullmatch(r"[a-f0-9]{64}", qualification.get("reportSha256", "")), "qualification report must be pinned")
    require(isinstance(data.get("revoked"), list) and len(data["revoked"]) <= 2000, "invalid revocations")
    for item in data["revoked"]:
        require(isinstance(item, str) and item.count("@") == 1, "invalid revocation")
        pack, version = item.split("@")
        identifier(pack)
        if version != "*": identifier(version)
    return data


def openssl(*args):
    result = subprocess.run(["openssl", *map(str, args)], capture_output=True, timeout=30,
                            env=crypto_environment())
    require(result.returncode == 0, "OpenSSL operation failed")
    return result.stdout


def crypto_environment():
    # Child crypto processes need the selected key file, never the entire key
    # map or a publication credential inherited from a runner environment.
    return {key: value for key, value in os.environ.items()
            if key not in ("MILKRUST_CATALOG_PRIVATE_KEYS", "GITHUB_TOKEN")}


def trust_root(path):
    data = decode(read(path, 16 * 1024))
    require(isinstance(data, dict) and data.get("schemaVersion") == 1 and isinstance(data.get("keys"), dict), "invalid trust root")
    require(data.get("status") == "active", "production trust root has not been activated")
    require(type(data.get("minimumSequence")) is int and 0 < data["minimumSequence"] <= 9007199254740991,
            "trust root is missing its release sequence floor")
    keys = data["keys"]
    require(0 < len(keys) <= 16 and type(data.get("threshold")) is int and 0 < data["threshold"] <= len(keys), "trust root is not provisioned")
    public_keys = set()
    for key_id, raw in keys.items():
        require(KEY_ID.fullmatch(key_id) and len(base64.b64decode(raw, validate=True)) == 32, "invalid Ed25519 public key")
        public_keys.add(base64.b64decode(raw, validate=True))
    require(len(public_keys) == len(keys), "each catalog key ID must pin a distinct public key")
    return data


def verify(envelope, trust, now, allow_expired=False, enforce_floor=True):
    require(len(envelope) <= MAX_ENVELOPE, "envelope too large")
    wrapper = decode(envelope)
    require(isinstance(wrapper, dict) and wrapper.get("schemaVersion") == 1, "unsupported envelope schema")
    payload = base64.b64decode(wrapper["payload"], validate=True)
    require(len(payload) <= MAX_PAYLOAD, "payload too large")
    require(isinstance(wrapper.get("signatures"), list) and len(wrapper["signatures"]) <= 16, "invalid signatures")
    accepted = set()
    with tempfile.TemporaryDirectory(prefix="milkrust-verify-") as scratch:
        folder = Path(scratch)
        message = folder / "message"
        message.write_bytes(DOMAIN + payload)
        for item in wrapper["signatures"]:
            require(isinstance(item, dict) and isinstance(item.get("keyId"), str) and isinstance(item.get("signature"), str), "malformed signature")
            key_id = item["keyId"]
            if key_id not in trust["keys"] or key_id in accepted:
                continue
            signature = base64.b64decode(item["signature"], validate=True)
            if len(signature) != 64: continue
            public = folder / "public.der"
            public.write_bytes(SPKI_PREFIX + base64.b64decode(trust["keys"][key_id], validate=True))
            sig = folder / "signature"
            sig.write_bytes(signature)
            result = subprocess.run(["openssl", "pkeyutl", "-verify", "-rawin", "-pubin", "-keyform", "DER", "-inkey", str(public), "-in", str(message), "-sigfile", str(sig)], capture_output=True, timeout=30, env=crypto_environment())
            if result.returncode == 0: accepted.add(key_id)
    require(len(accepted) >= trust["threshold"], "signature quorum not met")
    verified = validate_payload(payload, now, allow_expired)
    require(not enforce_floor or verified["sequence"] >= trust.get("minimumSequence", 1), "catalog predates this release trust root")
    return verified


def sign(payload, key_files, trust, now):
    validate_payload(payload, now)
    signatures = []
    with tempfile.TemporaryDirectory(prefix="milkrust-sign-") as scratch:
        message = Path(scratch) / "message"
        message.write_bytes(DOMAIN + payload)
        for key_id, key_file in sorted(key_files.items()):
            require(key_id in trust["keys"], "signing key is not in the release trust root")
            public = openssl("pkey", "-in", key_file, "-pubout", "-outform", "DER")
            require(public == SPKI_PREFIX + base64.b64decode(trust["keys"][key_id], validate=True), "private key does not match its pinned public key")
            raw = openssl("pkeyutl", "-sign", "-rawin", "-inkey", key_file, "-in", message)
            require(len(raw) == 64, "unexpected Ed25519 signature size")
            signatures.append({"keyId": key_id, "signature": base64.b64encode(raw).decode()})
    envelope = encode({"schemaVersion": 1, "payload": base64.b64encode(payload).decode(), "signatures": signatures})
    verify(envelope, trust, now)
    return envelope


def git_bytes(revision, path, limit):
    require(re.fullmatch(r"[a-f0-9]{40}", revision), "expected immutable git revision")
    object_name = f"{revision}:{path}"
    size = int(subprocess.check_output(["git", "cat-file", "-s", object_name], timeout=30))
    require(0 < size <= limit, "git content exceeds limit")
    return subprocess.check_output(["git", "show", object_name], timeout=30)


def archive_metadata(data):
    require(0 < len(data) <= MAX_ARCHIVE, "archive exceeds limit")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        infos = archive.infolist()
        require(0 < len(infos) <= 2500, "archive file limit exceeded")
        names, spellings, expanded = set(), {}, 0
        for item in infos:
            name = item.filename
            require(not item.is_dir() and len(name) <= 512 and not re.search(r'[\\:\x00-\x1f\x7f<>"|?*]', name), "unsafe archive member")
            parts = name.split("/")
            for i, part in enumerate(parts):
                require(part not in ("", ".", "..") and len(part) <= 240 and not part.endswith((" ", "."))
                        and not re.match(r"^(con|prn|aux|nul|com[0-9]|lpt[0-9])($|\.)", part, re.I), "unsafe portable path")
                prefix = "/".join(parts[:i+1])
                require(spellings.setdefault(prefix.lower(), prefix) == prefix, "archive case collision")
            require(name not in names and stat.S_IFMT(item.external_attr >> 16) != stat.S_IFLNK and not (item.flag_bits & 1), "duplicate, linked or encrypted archive member")
            names.add(name)
            require(item.file_size <= 16 * MAX_PAYLOAD, "archive member too large")
            expanded += item.file_size
        require(expanded <= MAX_EXPANDED and '.catalog-receipt.json' not in names, "expanded archive limit or reserved path")
        manifest = decode(archive.read("manifest.json"))
        integrity = decode(archive.read("integrity.json"))
        require(manifest.get("schemaVersion") == 1 and manifest.get("distributionStatus") == "redistributable" and manifest.get("license") in LICENSES, "uncleared archive manifest")
        require(integrity.get("schemaVersion") == 1 and integrity.get("packId") == manifest["id"] and integrity.get("packVersion") == manifest["version"], "integrity identity mismatch")
        records = integrity.get("files", [])
        indexed = {record["path"]: record for record in records}
        require(len(indexed) == len(records) and set(indexed) == names - {"integrity.json"}, "integrity index must cover each file once")
        for name, record in indexed.items():
            content = archive.read(name)
            require(record["size"] == len(content) and record["sha256"] == hashlib.sha256(content).hexdigest(), "archive integrity mismatch")
        licenses = manifest.get("licenseFiles", [])
        require(isinstance(licenses, list) and licenses and set(licenses) <= names and all(archive.read(name).strip() for name in licenses), "missing license notices")
        assets = manifest.get("assets", [])
        attributed = set()
        for asset in assets:
            require(asset.get("path") not in attributed and asset.get("license") in LICENSES and asset.get("author") and asset.get("licenseFile") in licenses, "missing per-asset attribution")
            require(isinstance(asset.get("sourceUrl"), str) and asset["sourceUrl"].startswith("https://"), "missing asset source")
            attributed.add(asset["path"])
        require(attributed == names - {"manifest.json", "integrity.json", *licenses}, "asset attribution does not cover the archive")
        presets = manifest.get("presets", [])
        kind = validate_pack_dependencies(manifest)
        require(isinstance(presets, list) and (kind == "textures" or len(presets) > 0)
                and len(presets) <= 2500 and all(isinstance(p, dict) and p.get("file") in names for p in presets), "invalid preset inventory")
        if kind == "textures":
            require(all(isinstance(t, dict) and t.get("file") in attributed for t in manifest["textures"]), "invalid texture inventory")
        return manifest, expanded


def prepare(config, sequence, days, now):
    require(type(sequence) is int and 0 < sequence <= 9007199254740991 and 1 <= days <= 31, "invalid sequence or lifetime")
    entries = []
    for item in config["packs"]:
        match = ARCHIVE_URL.fullmatch(item["archiveUrl"])
        require(match, "pack archive URL must pin an existing content commit")
        data = git_bytes(match[1], "content/downloads/" + match[2], MAX_ARCHIVE)
        manifest, expanded = archive_metadata(data)
        for key in ("id", "version", "license"):
            require(manifest[key] == item[key], "catalog/manifest identity mismatch")
        digest = hashlib.sha256(data).hexdigest()
        require(item.get("sha256") == digest and item.get("archiveBytes") == len(data) and item.get("expandedBytes") == expanded
                and item.get("presetCount") == len(manifest["presets"]), "reviewed catalog metadata differs from immutable archive")
        require(item.get("kind", "presets") == manifest.get("kind", "presets"), "reviewed pack kind differs from archive")
        qualification = qualify(item, manifest, data, git_bytes, decode)
        entries.append({"id": manifest["id"], "version": manifest["version"], "name": manifest["name"],
            "license": manifest["license"], "distributionStatus": "redistributable", "contentNotice": manifest["contentNotice"],
            "description": item.get("description", item.get("notes", manifest["name"])),
            "sourceUrl": item["sourceUrls"][0], "searchText": item.get("searchText", ""),
            "archiveUrl": item["archiveUrl"], "sha256": digest, "archiveBytes": len(data), "expandedBytes": expanded,
            "presetCount": len(manifest["presets"]), "qualification": qualification,
            **({"kind": "textures"} if manifest.get("kind") == "textures" else {})})
    payload = encode({"schemaVersion": 1, "sequence": sequence,
        "issuedAt": now.isoformat(timespec="seconds").replace("+00:00", "Z"),
        "expiresAt": (now + timedelta(days=days)).isoformat(timespec="seconds").replace("+00:00", "Z"),
        "entries": sorted(entries, key=lambda e: e["id"]), "revoked": sorted(set(config.get("revoked", [])))})
    validate_payload(payload, now)
    return payload


def api(method, path, token, body=None, missing=False):
    require(path.startswith("/repos/snapetech-org/MilkRust/"), "invalid publication repository")
    request = urllib.request.Request("https://api.github.com" + path, method=method,
        data=encode(body) if body is not None else None,
        headers={"Authorization": "Bearer " + token, "Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28", "Content-Type": "application/json"})
    # Do not forward a credential to redirects or include server bodies in errors.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args): return None
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=30) as response:
            data = response.read(8 * MAX_ENVELOPE + 1)
            require(len(data) <= 8 * MAX_ENVELOPE, "publication response exceeds limit")
            return decode(data)
    except urllib.error.HTTPError as error:
        if missing and error.code == 404: return None
        raise ValueError(f"catalog publication HTTP {error.code}") from None


def publish(envelope, trust, now, bootstrap=False):
    next_catalog = verify(envelope, trust, now)
    token = os.environ.get("GITHUB_TOKEN", "")
    require(token, "publication token is not configured")
    prefix = "/repos/snapetech-org/MilkRust"
    ref = api("GET", prefix + "/git/ref/heads/catalog", token, missing=True)
    parent = ref["object"]["sha"] if ref else None
    require(parent or bootstrap, "first catalog publication requires explicit bootstrap")
    base_tree = None
    if parent:
        commit = api("GET", prefix + "/git/commits/" + parent, token)
        base_tree = commit["tree"]["sha"]
        old_file = api("GET", prefix + "/contents/visualizer-catalog.json?ref=" + parent, token)
        # Authenticate history even when a new app root advances its sequence
        # floor beyond the old publication. The new envelope still obeys it.
        previous = verify(base64.b64decode(old_file["content"]), trust, now, allow_expired=True, enforce_floor=False)
        require(next_catalog["sequence"] > previous["sequence"], "catalog sequence must increase")
        require(set(previous["revoked"]) <= set(next_catalog["revoked"]), "revocations are cumulative; publish a new pack version to restore")
    blob = api("POST", prefix + "/git/blobs", token, {"encoding": "base64", "content": base64.b64encode(envelope).decode()})
    tree_body = {"tree": [{"path": "visualizer-catalog.json", "mode": "100644", "type": "blob", "sha": blob["sha"]}]}
    if base_tree: tree_body["base_tree"] = base_tree
    tree = api("POST", prefix + "/git/trees", token, tree_body)
    commit = api("POST", prefix + "/git/commits", token, {"message": f"Publish signed catalog {next_catalog['sequence']} [skip ci]", "tree": tree["sha"], "parents": [parent] if parent else []})
    if parent:
        # Non-force fast-forward is the server-side concurrency check. A racing
        # sibling publication cannot silently replace this branch's history.
        api("PATCH", prefix + "/git/refs/heads/catalog", token, {"sha": commit["sha"], "force": False})
    else:
        api("POST", prefix + "/git/refs", token, {"ref": "refs/heads/catalog", "sha": commit["sha"]})
    published = api("GET", prefix + "/contents/visualizer-catalog.json?ref=" + commit["sha"], token)
    require(base64.b64decode(published["content"]) == envelope, "published envelope differs from signed bytes")
    print(json.dumps({"sequence": next_catalog["sequence"], "commit": commit["sha"], "sha256": hashlib.sha256(envelope).hexdigest()}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "sign", "verify", "publish"))
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--trust", type=Path, default=Path("content/catalog-trust.json"))
    parser.add_argument("--sequence", type=int)
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--key", action="append", default=[], metavar="KEY_ID=PRIVATE_KEY_FILE")
    parser.add_argument("--bootstrap", action="store_true")
    args = parser.parse_args()
    now = datetime.now(timezone.utc)
    data = read(args.input, MAX_ENVELOPE)
    result = None
    if args.command == "prepare":
        result = prepare(decode(data), args.sequence, args.days, now)
    else:
        trust = trust_root(args.trust)
        if args.command == "sign":
            key_files = {}
            for argument in args.key:
                key_id, filename = argument.split("=", 1)
                require(KEY_ID.fullmatch(key_id) and key_id not in key_files, "invalid or duplicate key ID")
                key_files[key_id] = Path(filename)
            # JIT runner secret: JSON map of key ID to PEM. Temporary key files
            # stay mode 0600 inside a 0700 directory and are never artifacts.
            with tempfile.TemporaryDirectory(prefix="milkrust-keys-") as scratch:
                secrets = decode(os.environ.pop("MILKRUST_CATALOG_PRIVATE_KEYS", "{}"))
                require(isinstance(secrets, dict) and len(secrets) <= 16, "invalid signing configuration")
                for key_id, pem in secrets.items():
                    require(KEY_ID.fullmatch(key_id) and key_id not in key_files and isinstance(pem, str) and len(pem) <= 16384, "invalid signing configuration")
                    filename = Path(scratch) / key_id
                    with filename.open("x", opener=lambda path, flags: os.open(path, flags, 0o600)) as key_file:
                        key_file.write(pem)
                    key_files[key_id] = filename
                result = sign(data, key_files, trust, now)
        elif args.command == "verify":
            verified = verify(data, trust, now)
            print(json.dumps({"sequence": verified["sequence"], "entries": len(verified["entries"])}))
        else:
            publish(data, trust, now, args.bootstrap)
    if result is not None:
        require(args.output is not None, "--output is required")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.output.with_name(args.output.name + ".partial")
        with temporary.open("wb") as target:
            target.write(result); target.flush(); os.fsync(target.fileno())
        os.replace(temporary, args.output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        # Never print a subprocess command/environment or private key content.
        raise SystemExit(f"catalog operation failed ({type(error).__name__}): {error}")
