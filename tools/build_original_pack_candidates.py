#!/usr/bin/env python3
"""Package attributed sources and create pending evidence inventories on private JIT.

Does not render, sign, publish, approve rights, or promote a catalog entry.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from catalog_qualification import TARGETS

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ("content/original-packs", "content/licensed-packs")


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT, timeout=60)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source-root", choices=SOURCES, default=SOURCES[0])
    args = parser.parse_args()
    revision = args.source_revision
    if re.fullmatch(r"[a-f0-9]{40}", revision) is None:
        raise ValueError("source revision must be an immutable full commit")
    if args.output.exists() or args.output.is_symlink():
        raise ValueError("output must be a new directory")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    entries = git("ls-tree", "-r", revision, "--", args.source_root).decode().splitlines()
    records = {}
    for entry in entries:
        meta, path = entry.split("\t", 1)
        mode, kind, sha = meta.split()
        if mode != "100644" or kind != "blob" or not re.fullmatch(r"[a-zA-Z0-9_./-]+", path):
            raise ValueError("original pack sources must be portable regular files")
        if int(git("cat-file", "-s", sha)) > 1024 * 1024:
            raise ValueError("original pack source exceeds 1 MiB")
        records[path] = sha
    manifests = sorted(p for p in records if p.endswith("/manifest.json"))
    if not manifests:
        raise ValueError("revision contains no original pack manifests")
    with tempfile.TemporaryDirectory(prefix="milkrust-original-", dir=args.output.parent) as tmp:
        staging = Path(tmp) / "delivery"
        (staging / "archives").mkdir(parents=True)
        (staging / "qualification").mkdir()
        packs = []
        for manifest_path in manifests:
            prefix = manifest_path.rsplit("/", 1)[0] + "/"
            folder = Path(tmp) / prefix.rsplit("/", 2)[1]
            for path, sha in records.items():
                if not path.startswith(prefix):
                    continue
                output = folder / path[len(prefix):]
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(git("cat-file", "blob", sha))
            manifest_file = folder / "manifest.json"
            manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
            if not re.fullmatch(r"milkrust-[a-z0-9-]+", manifest["id"]):
                raise ValueError("invalid original pack identity")
            immutable = f"https://github.com/snapetech-org/MilkRust/tree/{revision}/{prefix.rstrip('/')}"
            original = args.source_root == SOURCES[0]
            if original:
                manifest["sourceUrls"] = [immutable]
            for asset in manifest["assets"]:
                if original:
                    asset["sourceUrl"] = f"https://github.com/snapetech-org/MilkRust/blob/{revision}/{prefix}{asset['path']}"
                else:
                    source_revision = asset.get("sourceRevision", "")
                    if not re.fullmatch(r"[a-f0-9]{40}", source_revision) or f"/blob/{source_revision}/" not in asset.get("sourceUrl", ""):
                        raise ValueError("licensed assets require immutable upstream provenance")
                    relative = asset["path"]
                    if relative.startswith("/") or ".." in relative.split("/") or "\\" in relative:
                        raise ValueError("unsafe licensed asset path")
                    data = (folder / relative).read_bytes()
                    blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
                    if blob != asset.get("sourceGitBlob") or hashlib.sha256(data).hexdigest() != asset.get("sha256"):
                        raise ValueError("licensed source differs from its provenance receipt")
            manifest_file.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            archive = staging / "archives" / f"{manifest['id']}-{manifest['version']}.rmpack"
            run = subprocess.run([sys.executable, str(ROOT / "tools/package-rmpack.py"), str(folder), str(archive)],
                                 cwd=ROOT, capture_output=True, text=True, timeout=120, check=True)
            summary = json.loads(run.stdout)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            if digest != summary["sha256"]:
                raise ValueError("archive differs from package summary")
            inventory = [{"file": p["file"], "sha256": hashlib.sha256((folder / p["file"]).read_bytes()).hexdigest(),
                          "status": "pending", "evidence": None} for p in manifest["presets"]]
            pending = {"schemaVersion": 1, "status": "pending", "policy": "all-native-hosts-v1",
                       "pack": {"id": manifest["id"], "version": manifest["version"], "sha256": digest},
                       "sourceRevision": revision,
                       "rightsReview": {"status": "pending", "reviewer": None,
                           "basis": ("Original sources with explicit per-file CC0 dedication; verify scope and provenance before approval." if original else
                               "Explicit upstream license notice, immutable source and per-file attribution; review exact archive scope and notices before approval."), "evidence": None},
                       "targets": [{"id": target, "status": "pending", "presets": inventory,
                           "engineRevision": None, "hostRevision": None, "engineArtifactSha256": None,
                           "hostArtifactSha256": None, "device": None, "runUrl": None,
                           "runnerPolicy": "private-ephemeral-jit"} for target in sorted(TARGETS)]}
            report_name = f"{manifest['id']}.pending.json"
            (staging / "qualification" / report_name).write_text(json.dumps(pending, indent=2) + "\n", encoding="utf-8")
            packs.append({**summary, "archive": f"archives/{archive.name}", "sourceRevision": revision,
                          "compatibilityStatus": "authored-unqualified" if original else "upstream-unqualified", "publicationEligible": False,
                          "pendingQualification": f"qualification/{report_name}"})
        (staging / "candidate-index.json").write_text(json.dumps({"schemaVersion": 1, "sourceRevision": revision,
            "publicationEligible": False, "packs": packs, "rendered": 0, "qualified": 0}, indent=2) + "\n", encoding="utf-8")
        # Same-filesystem rename publishes only the fully assembled candidates.
        staging.rename(args.output)
    print(json.dumps({"candidatePacks": len(packs), "qualified": 0, "publicationEligible": False}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Original-pack preparation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
