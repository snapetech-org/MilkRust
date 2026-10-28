#!/usr/bin/env python3
"""Build a reproducible, redistribution-cleared MilkRust .rmpack archive."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath


EPOCH = (1980, 1, 1, 0, 0, 0)
MAX_FILES = 20_000
MAX_EXPANDED_BYTES = 512 * 1024 * 1024


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
    parser.add_argument("pack_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    root = args.pack_dir.resolve(strict=True)
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schemaVersion") != 1:
        raise ValueError("only manifest schema version 1 can be packaged")
    if manifest.get("distributionStatus") != "redistributable":
        raise ValueError("pack manifest must explicitly declare redistributable status")
    if not manifest.get("license") or not manifest.get("licenseFiles"):
        raise ValueError("pack must declare a license and ship its license notice")
    for license_file in manifest["licenseFiles"]:
        target = root / safe_relative_path(license_file)
        if not target.is_file():
            raise ValueError(f"missing license notice: {license_file}")

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
        "expandedBytes": total_bytes,
    }
    output_files = files + [("integrity.json", None, (json.dumps(integrity, indent=2, sort_keys=True) + "\n").encode())]
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
        "expandedBytes": total_bytes,
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
