#!/usr/bin/env python3
"""Build deterministic MilkRust .rmpack archives.

The default mode requires a redistributable manifest. --review-candidate only
packages explicitly non-promotable candidates whose rights still need review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit
sys.path.insert(0, str(Path(__file__).resolve().parent))
from validate_pack_dependencies import validate_pack_dependencies


EPOCH = (1980, 1, 1, 0, 0, 0)
MAX_FILES = 20_000
MAX_EXPANDED_BYTES = 512 * 1024 * 1024
VENDORABLE_LICENSES = {
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "CC-BY-3.0",
    "CC-BY-4.0", "CC-BY-SA-3.0", "CC-BY-SA-4.0", "CC0-1.0",
    "GPL-2.0-only", "GPL-2.0-or-later", "GPL-3.0-only", "GPL-3.0-or-later",
    "ISC", "LGPL-2.1-only", "LGPL-2.1-or-later", "LGPL-3.0-only",
    "LGPL-3.0-or-later", "MIT", "Unlicense", "Zlib",
}


def safe_relative_path(path: str) -> str:
    normalized = path.replace("\\", "/")
    pure = PurePosixPath(normalized)
    if not normalized or pure.is_absolute() or any(part in ("", ".", "..") for part in pure.parts):
        raise ValueError(f"unsafe pack path: {path!r}")
    if ":" in pure.parts[0] or "\x00" in normalized:
        raise ValueError(f"unsafe pack path: {path!r}")
    return pure.as_posix()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--review-candidate", action="store_true",
                        help="package a rights-review-required candidate that cannot enter the public catalog")
    parser.add_argument("pack_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    root = args.pack_dir.resolve(strict=True)
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict):
        raise ValueError("pack manifest must be a JSON object")
    validate_pack_dependencies(manifest)
    if manifest.get("schemaVersion") != 1:
        raise ValueError("only manifest schema version 1 can be packaged")
    if not isinstance(manifest.get("id"), str) or not manifest["id"].strip():
        raise ValueError("pack must declare a non-empty id")
    if not isinstance(manifest.get("version"), str) or not manifest["version"].strip():
        raise ValueError("pack must declare a non-empty version")
    if args.review_candidate:
        if (manifest.get("distributionStatus") != "rights-review-required"
                or manifest.get("rightsReviewStatus") != "required"):
            raise ValueError("review-candidate mode requires an explicitly rights-review-required manifest")
    elif manifest.get("distributionStatus") != "redistributable":
        raise ValueError("pack manifest must explicitly declare redistributable status")
    license_files = manifest.get("licenseFiles")
    if manifest.get("license") not in VENDORABLE_LICENSES or not isinstance(license_files, list) or not license_files:
        raise ValueError("pack must declare a license and ship its license notice")
    if not isinstance(manifest.get("contentNotice"), str) or not manifest["contentNotice"].strip():
        raise ValueError("pack must include a non-empty contentNotice")
    if not isinstance(manifest.get("sourceUrls"), list) or not manifest["sourceUrls"]:
        raise ValueError("pack must include sourceUrls")
    for source_url in manifest["sourceUrls"]:
        parsed_url = urlsplit(source_url) if isinstance(source_url, str) else None
        if parsed_url is None or parsed_url.scheme != "https" or not parsed_url.netloc:
            raise ValueError("pack sourceUrls must contain HTTPS URLs")

    license_paths = set()
    for license_file in license_files:
        if not isinstance(license_file, str):
            raise ValueError("licenseFiles entries must be relative paths")
        relative = safe_relative_path(license_file)
        target = root / relative
        if not target.is_file():
            raise ValueError(f"missing license notice: {license_file}")
        if not target.read_bytes().strip():
            raise ValueError(f"license notice is empty: {license_file}")
        if relative in license_paths:
            raise ValueError(f"duplicate license notice path: {relative}")
        license_paths.add(relative)

    assets = manifest.get("assets")
    if not isinstance(assets, list) or not assets:
        raise ValueError("pack must declare an assets list with per-file attribution")
    asset_paths = set()
    for index, asset in enumerate(assets):
        label = f"assets[{index}]"
        if not isinstance(asset, dict):
            raise ValueError(f"{label} must be an object")
        if not isinstance(asset.get("path"), str):
            raise ValueError(f"{label} must declare a relative path")
        relative = safe_relative_path(asset["path"])
        if relative in asset_paths:
            raise ValueError(f"duplicate asset path: {relative}")
        asset_paths.add(relative)
        if relative in license_paths or relative == "manifest.json":
            raise ValueError(f"{label} must reference content, not pack metadata")
        if not isinstance(asset.get("kind"), str) or not asset["kind"].strip():
            raise ValueError(f"{label} must declare a kind")
        if args.review_candidate:
            if asset.get("license") != "NOASSERTION" or asset.get("rightsStatus") != "review-required":
                raise ValueError(f"{label} must remain NOASSERTION until rights review is complete")
        elif asset.get("license") not in VENDORABLE_LICENSES:
            raise ValueError(f"{label} must use an explicitly redistributable license")
        if not isinstance(asset.get("author"), str) or not asset["author"].strip():
            raise ValueError(f"{label} must declare its author or rights holder")
        source_url = asset.get("sourceUrl")
        parsed_url = urlsplit(source_url) if isinstance(source_url, str) else None
        if parsed_url is None or parsed_url.scheme != "https" or not parsed_url.netloc:
            raise ValueError(f"{label} must declare an HTTPS sourceUrl")
        if not args.review_candidate:
            if not isinstance(asset.get("licenseFile"), str):
                raise ValueError(f"{label} must declare a relative licenseFile path")
            notice_path = safe_relative_path(asset["licenseFile"])
            if notice_path not in license_paths:
                raise ValueError(f"{label} must point to a shipped licenseFile")
        if not (root / relative).is_file():
            raise ValueError(f"missing asset: {relative}")

    files: list[tuple[str, Path, bytes]] = []
    total_bytes = 0
    for candidate in sorted(root.rglob("*")):
        if candidate.is_symlink():
            raise ValueError(f"symlinks are not allowed in packs: {candidate}")
        if not candidate.is_file():
            continue
        relative = safe_relative_path(candidate.relative_to(root).as_posix())
        data = candidate.read_bytes()
        total_bytes += len(data)
        files.append((relative, candidate, data))
    content_paths = {
        name for name, _, _ in files
        if name != "manifest.json" and name not in license_paths
    }
    missing_attribution = content_paths - asset_paths
    missing_files = asset_paths - {name for name, _, _ in files}
    if missing_attribution:
        raise ValueError("content files missing per-file attribution: " + ", ".join(sorted(missing_attribution)))
    if missing_files:
        raise ValueError("asset records reference missing files: " + ", ".join(sorted(missing_files)))
    if len(files) > MAX_FILES:
        raise ValueError(f"pack contains more than {MAX_FILES} files")
    if total_bytes > MAX_EXPANDED_BYTES:
        raise ValueError(f"expanded pack exceeds {MAX_EXPANDED_BYTES} bytes")

    integrity = {
        "schemaVersion": 1,
        "packId": manifest["id"],
        "packVersion": manifest["version"],
        "files": [
            {"path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            for name, _, data in files
        ],
        "contentBytes": total_bytes,
    }
    integrity_bytes = (json.dumps(integrity, indent=2, sort_keys=True) + "\n").encode()
    expanded_archive_bytes = total_bytes + len(integrity_bytes)
    if expanded_archive_bytes > MAX_EXPANDED_BYTES:
        raise ValueError(f"expanded pack exceeds {MAX_EXPANDED_BYTES} bytes")
    output_files = files + [("integrity.json", None, integrity_bytes)]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".partial")
    with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, _, data in output_files:
            info = zipfile.ZipInfo(name, EPOCH)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            archive.writestr(info, data)
    os.replace(temporary, args.output)
    archive_bytes = args.output.read_bytes()
    print(json.dumps({
        "path": str(args.output),
        "packId": manifest["id"],
        "version": manifest["version"],
        "license": manifest["license"],
        "fileCount": len(files),
        "presetCount": len(manifest.get("presets", [])),
        "expandedBytes": expanded_archive_bytes,
        "archiveBytes": len(archive_bytes),
        "sha256": hashlib.sha256(archive_bytes).hexdigest(),
    }, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"packaging failed: {error}", file=sys.stderr)
        raise SystemExit(2)
