#!/usr/bin/env python3
"""Prepare unqualified, rights-review-required pack archives for review.

This creates non-promotable candidate .rmpack files and a staging catalog only. It never edits
content/downloads/catalog.json, runs preset code, renders presets, or claims
compatibility qualification. Run it only on an approved private ephemeral JIT
runner.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = ROOT / "content/third-party/butterchurn-presets-2.4.7"
COMMIT = re.compile(r"[a-f0-9]{40}\Z")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def run(command: list[str]) -> str:
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    if result.returncode:
        detail = result.stderr.strip() or result.stdout.strip() or "subcommand failed"
        raise RuntimeError(detail[-4000:])
    return result.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--source-revision", required=True,
                        help="immutable full MilkRust commit containing the vendored source")
    parser.add_argument("--output", required=True, type=Path,
                        help="new directory for candidate archives and reports")
    parser.add_argument("--pack-size", type=int, default=200)
    args = parser.parse_args()

    require(COMMIT.fullmatch(args.source_revision) is not None,
            "source revision must be a full lowercase 40-character commit")
    require(1 <= args.pack_size <= 500, "pack size must be between 1 and 500")
    require(not args.output.exists(), "output must be a new directory")
    args.output.parent.mkdir(parents=True, exist_ok=True)

    temporary = Path(tempfile.mkdtemp(prefix="milkrust-pack-candidates-", dir=args.output.parent))
    staging = temporary / "candidate"
    conversion = staging / "conversion"
    archives = staging / "archives"
    staging.mkdir()
    archives.mkdir()
    try:
        run([
            sys.executable, str(ROOT / "tools/convert_butterchurn.py"),
            "--source", str(args.source), "--source-revision", args.source_revision,
            "--pack-size", str(args.pack_size), "--output", str(conversion),
        ])
        report_path = conversion / "conversion-report.json"
        report = json.loads(report_path.read_text(encoding="utf-8"))
        rows = report.get("rows")
        require(isinstance(rows, list) and rows, "conversion report has no source accounting rows")
        require(report.get("discovered") == len(rows), "discovered count differs from source accounting")
        require(report.get("converted", -1) + report.get("rejected", -1) == len(rows),
                "converted and rejected counts do not account for every source")
        require(report.get("rendered") == 0 and report.get("qualified") == 0,
                "converter must not claim render or qualification evidence")
        require(all(row.get("rendered") is False and row.get("qualified") is False for row in rows),
                "a source row unexpectedly claims rendering or qualification")

        packs = []
        for pack_dir in sorted(path for path in conversion.iterdir() if path.is_dir()):
            manifest_path = pack_dir / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            archive_path = archives / f"{manifest['id']}-{manifest['version']}.rmpack"
            summary = json.loads(run([
                sys.executable, str(ROOT / "tools/package-rmpack.py"), "--review-candidate",
                str(pack_dir), str(archive_path),
            ]))
            archive_bytes = archive_path.read_bytes()
            digest = hashlib.sha256(archive_bytes).hexdigest()
            require(summary.get("sha256") == digest and summary.get("packId") == manifest.get("id"),
                    "packager summary differs from archive bytes or manifest identity")
            packs.append({
                "id": manifest["id"],
                "name": manifest["name"],
                "version": manifest["version"],
                "license": manifest["license"],
                "distributionStatus": "rights-review-required",
                "rightsReviewStatus": "required",
                "rightsBasis": "the source package declares MIT, but per-preset rights-holder chains are not confirmed",
                "archive": archive_path.relative_to(staging).as_posix(),
                "sha256": digest,
                "archiveBytes": len(archive_bytes),
                "expandedBytes": summary["expandedBytes"],
                "fileCount": summary["fileCount"],
                "presetCount": summary["presetCount"],
                "compatibilityStatus": "converted-unqualified",
                "rendered": False,
                "qualified": False,
            })

        candidate_catalog = {
            "schemaVersion": 1,
            "status": "rights-and-qualification-review-candidates",
            "publicationEligible": False,
            "generatedBy": "tools/build_cleared_pack_candidates.py",
            "sourceRevision": args.source_revision,
            "sourceCorpusSha256": report["sourceCorpusSha256"],
            "conversionReport": "conversion/conversion-report.json",
            "discovered": report["discovered"],
            "converted": report["converted"],
            "rejected": report["rejected"],
            "rendered": 0,
            "qualified": 0,
            "packs": packs,
            "promotionBlockers": [
                "Confirm that the upstream MIT grant covers every underlying preset and transformed derivative; record independent per-asset rights and attribution review.",
                "Per-preset reference comparison, rendering, audio-sync, resource-limit and lifecycle evidence.",
                "All required physical-host qualification targets from content/qualification/README.md.",
                "Immutable archive commit, signed-catalog metadata review and production trust provisioning.",
            ],
            "notice": "Candidate archives are not listed in the in-app download catalog. Every preset remains NOASSERTION and each pack is rights-review-required. Conversion acceptance is not a rights grant, render compatibility or qualification.",
        }
        (staging / "candidate-catalog.json").write_text(
            json.dumps(candidate_catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (staging / "README.txt").write_text(
            "MilkRust preset pack review-candidate artifact\n"
            "\n"
            "This artifact contains deterministic, non-promotable .rmpack candidates and full source accounting.\n"
            "Every asset remains NOASSERTION until independent rights review is complete.\n"
            "It is not a production download catalog. No preset was executed or rendered.\n"
            "Review candidate-catalog.json and conversion/conversion-report.json before any\n"
            "qualification work. The bounded numeric and shader conversion profile requires MilkRust and remains unqualified.\n",
            encoding="utf-8",
        )
        os.replace(staging, args.output)
        print(json.dumps({
            "output": str(args.output),
            "discovered": report["discovered"],
            "converted": report["converted"],
            "rejected": report["rejected"],
            "candidatePacks": len(packs),
            "rendered": 0,
            "qualified": 0,
            "publicationEligible": False,
        }, indent=2))
    finally:
        shutil.rmtree(temporary, ignore_errors=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, json.JSONDecodeError) as error:
        raise SystemExit(f"candidate preparation failed: {error}") from None
