"""Check reviewed, immutable pack qualification records before catalog signing.

This validates evidence records. It does not run a renderer, infer a pass from a
conversion, or certify that an uploaded report is truthful. The report and its
evidence must receive release review before their immutable commit is promoted.
"""
from __future__ import annotations

import hashlib
import io
import re
import zipfile

TARGETS = frozenset({
    "linux-x64", "linux-arm64", "windows-x64", "windows-arm64",
    "macos-x64", "macos-arm64", "ios-arm64", "tvos-arm64",
    "android-arm64", "android-armv7", "android-x64", "webos-arm", "tizen-arm",
})
HEX = re.compile(r"[a-f0-9]{64}\Z")
COMMIT = re.compile(r"[a-f0-9]{40}\Z")
# This narrow exception is an authored fixture, never a qualification claim.
FIXTURE = ("milkrust-sample-pack", "0.1.0",
           "175a042deb4edb224055a6334f67a1c492f32e447dc3c90a54f22849c43a27df")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def text(value, limit=512):
    require(isinstance(value, str) and value.strip() and len(value) <= limit
            and not any(ord(c) < 32 for c in value), "invalid qualification text")
    return value


def commit(value):
    require(isinstance(value, str) and COMMIT.fullmatch(value), "qualification needs an immutable source revision")


def digest(value):
    require(isinstance(value, str) and HEX.fullmatch(value), "qualification needs a SHA-256 digest")


def evidence_record(record, revision, git_bytes, decode):
    require(isinstance(record, dict), "missing qualification evidence")
    path = record.get("path", "")
    require(isinstance(path, str) and re.fullmatch(
        r"content/qualification/evidence/[A-Za-z0-9_-]+\.json", path), "invalid evidence path")
    digest(record.get("sha256"))
    data = git_bytes(revision, path, 256 * 1024)
    require(hashlib.sha256(data).hexdigest() == record["sha256"], "evidence digest mismatch")
    parsed = decode(data)
    require(isinstance(parsed, dict) and parsed.get("schemaVersion") == 1, "invalid evidence schema")
    return parsed


def qualify(item, manifest, archive, git_bytes, decode):
    archive_digest = hashlib.sha256(archive).hexdigest()
    identity = (manifest["id"], manifest["version"], archive_digest)
    if identity == FIXTURE:
        require(item.get("compatibilityStatus") == "sample-fixture-only", "fixture must not claim qualification")
        return {"status": "sample-fixture-only", "targets": []}
    require(item.get("compatibilityStatus") == "qualified", "broader pack is not qualified for catalog delivery")
    pin = item.get("qualification")
    require(isinstance(pin, dict), "pack is missing its immutable qualification report")
    revision, path = pin.get("revision"), pin.get("path")
    commit(revision)
    require(isinstance(path, str) and re.fullmatch(
        r"content/qualification/reports/[A-Za-z0-9_-]+\.json", path), "invalid qualification report path")
    digest(pin.get("sha256"))
    raw = git_bytes(revision, path, 4 * 1024 * 1024)
    require(hashlib.sha256(raw).hexdigest() == pin["sha256"], "qualification report digest mismatch")
    report = decode(raw)
    require(isinstance(report, dict) and report.get("schemaVersion") == 1
            and report.get("status") == "qualified", "qualification did not pass")
    require(report.get("pack") == {"id": identity[0], "version": identity[1], "sha256": identity[2]},
            "qualification belongs to a different archive")
    require(report.get("policy") == "all-native-hosts-v1", "unknown qualification coverage policy")
    rights = report.get("rightsReview")
    require(isinstance(rights, dict) and rights.get("status") == "approved", "rights review is missing")
    text(rights.get("reviewer")); text(rights.get("basis"), 4096)
    legal = evidence_record(rights.get("evidence"), revision, git_bytes, decode)
    require(legal.get("packSha256") == archive_digest and legal.get("status") == "approved",
            "rights evidence does not cover this pack")
    with zipfile.ZipFile(io.BytesIO(archive)) as zipped:
        expected = {p["file"]: hashlib.sha256(zipped.read(p["file"])).hexdigest()
                    for p in manifest["presets"]}
    require(len(expected) == len(manifest["presets"]), "duplicate preset in qualification inventory")
    targets = report.get("targets")
    require(isinstance(targets, list) and len(targets) == len(TARGETS), "incomplete native host matrix")
    covered = set()
    for target in targets:
        require(isinstance(target, dict) and target.get("id") in TARGETS
                and target["id"] not in covered, "duplicate or unexpected qualification target")
        covered.add(target["id"])
        commit(target.get("engineRevision")); commit(target.get("hostRevision"))
        digest(target.get("engineArtifactSha256")); digest(target.get("hostArtifactSha256"))
        device = target.get("device")
        require(isinstance(device, dict) and device.get("physical") is True, "physical device evidence is required")
        for field in ("model", "os", "gpu", "driver"):
            text(device.get(field))
        require(isinstance(target.get("runUrl"), str) and re.fullmatch(
            r"https://github\.com/snapetech-org/(MilkRust|PlezyNG)/actions/runs/[1-9][0-9]*",
            target["runUrl"]), "qualification must identify a private JIT run")
        require(target.get("runnerPolicy") == "private-ephemeral-jit", "unapproved qualification runner")
        results = target.get("presets")
        require(isinstance(results, list) and len(results) == len(expected), "qualification omitted presets")
        seen = set()
        for result in results:
            require(isinstance(result, dict), "invalid preset qualification")
            name = result.get("file")
            require(isinstance(name, str) and name in expected and name not in seen,
                    "unexpected or duplicate qualified preset")
            seen.add(name)
            require(result.get("sha256") == expected[name] and result.get("status") == "pass",
                    "preset differs or did not pass")
            evidence = evidence_record(result.get("evidence"), revision, git_bytes, decode)
            require(evidence.get("packSha256") == archive_digest and evidence.get("presetSha256") == expected[name]
                    and evidence.get("target") == target["id"]
                    and evidence.get("engineRevision") == target["engineRevision"]
                    and evidence.get("hostRevision") == target["hostRevision"]
                    and evidence.get("engineArtifactSha256") == target["engineArtifactSha256"]
                    and evidence.get("hostArtifactSha256") == target["hostArtifactSha256"], "evidence identity mismatch")
            for field in ("render", "referenceComparison", "audioSync", "resourceLimits", "lifecycle"):
                check = evidence.get(field)
                require(isinstance(check, dict) and check.get("status") == "pass", "qualification check is missing or failed")
                text(check.get("method"), 4096)
            render = evidence["render"]
            require(type(render.get("frames")) is int and render["frames"] >= 300,
                    "qualification needs at least 300 rendered frames per preset")
            digest(render.get("captureSha256")); digest(render.get("audioFixtureSha256"))
            comparison = evidence["referenceComparison"]
            text(comparison.get("referenceVersion")); text(comparison.get("reviewer"))
            digest(comparison.get("captureSha256"))
            # Digests identify retained run artifacts; reviewers must inspect
            # those captures. This metadata verifier cannot inspect GPU output.
    require(covered == TARGETS, "native host coverage is incomplete")
    return {"status": "qualified", "targets": sorted(covered),
            "reportUrl": f"https://raw.githubusercontent.com/snapetech-org/MilkRust/{revision}/{path}",
            "reportSha256": pin["sha256"]}
