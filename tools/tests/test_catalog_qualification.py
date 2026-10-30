"""Synthetic metadata cases only; these are not qualification evidence."""
import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from catalog_qualification import TARGETS, qualify


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.revision = "a" * 40
        self.files = {}
        self.preset = b"[preset00]\nfDecay=0.98\n"
        self.preset_sha = hashlib.sha256(self.preset).hexdigest()
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w") as archive:
            archive.writestr("one.milk", self.preset)
        self.archive = stream.getvalue()
        self.sha = hashlib.sha256(self.archive).hexdigest()
        self.manifest = {"id": "test-only", "version": "1", "presets": [{"file": "one.milk"}]}
        rights = self.save("content/qualification/evidence/rights.json",
                           {"schemaVersion": 1, "packSha256": self.sha, "status": "approved"})
        self.report = {"schemaVersion": 1, "status": "qualified", "policy": "all-native-hosts-v1",
                       "pack": {"id": "test-only", "version": "1", "sha256": self.sha},
                       "rightsReview": {"status": "approved", "reviewer": "TEST ONLY",
                                        "basis": "Synthetic unit fixture", "evidence": rights}, "targets": []}
        for target in sorted(TARGETS):
            evidence = {"schemaVersion": 1, "packSha256": self.sha, "presetSha256": self.preset_sha,
                        "target": target, "engineRevision": self.revision, "hostRevision": self.revision,
                        "engineArtifactSha256": "e" * 64, "hostArtifactSha256": "f" * 64}
            for field in ("render", "referenceComparison", "audioSync", "resourceLimits", "lifecycle"):
                evidence[field] = {"status": "pass", "method": "Synthetic test, never actually rendered"}
            evidence["render"].update(frames=300, captureSha256="b" * 64, audioFixtureSha256="c" * 64)
            evidence["referenceComparison"].update(referenceVersion="TEST", reviewer="TEST", captureSha256="d" * 64)
            pin = self.save(f"content/qualification/evidence/{target}.json", evidence)
            self.report["targets"].append({"id": target, "engineRevision": self.revision,
                "hostRevision": self.revision, "engineArtifactSha256": "e" * 64,
                "hostArtifactSha256": "f" * 64, "runnerPolicy": "private-ephemeral-jit",
                "runUrl": "https://github.com/snapetech-org/MilkRust/actions/runs/1",
                "device": {"physical": True, "model": "TEST", "os": "TEST", "gpu": "TEST", "driver": "TEST"},
                "presets": [{"file": "one.milk", "sha256": self.preset_sha, "status": "pass", "evidence": pin}]})

    def save(self, path, data):
        raw = json.dumps(data).encode()
        self.files[path] = raw
        return {"path": path, "sha256": hashlib.sha256(raw).hexdigest()}

    def check(self, report=None):
        pin = self.save("content/qualification/reports/test-only.json", report or self.report)
        pin["revision"] = self.revision
        item = {"compatibilityStatus": "qualified", "qualification": pin}
        def git_bytes(revision, path, limit):
            self.assertEqual(revision, self.revision)
            raw = self.files[path]
            self.assertLessEqual(len(raw), limit)
            return raw
        return qualify(item, self.manifest, self.archive, git_bytes, json.loads)

    def test_complete_record_describes_only_its_pinned_archive(self):
        self.assertEqual(self.check()["targets"], sorted(TARGETS))
        self.report["pack"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "different archive"):
            self.check()

    def test_missing_or_duplicate_target_does_not_satisfy_matrix(self):
        missing = copy.deepcopy(self.report)
        missing["targets"].pop()
        with self.assertRaisesRegex(ValueError, "matrix"):
            self.check(missing)
        self.report["targets"][-1] = self.report["targets"][0]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.check()

    def test_converted_inventory_cannot_replace_a_render_result(self):
        self.report["targets"][0]["presets"][0]["status"] = "converted-unqualified"
        with self.assertRaisesRegex(ValueError, "did not pass"):
            self.check()

    def test_modified_evidence_and_wrong_preset_digest_are_rejected(self):
        result = self.report["targets"][0]["presets"][0]
        result["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "differs"):
            self.check()
        result["sha256"] = self.preset_sha
        self.files[result["evidence"]["path"]] += b" "
        with self.assertRaisesRegex(ValueError, "evidence digest"):
            self.check()

    def test_nonphysical_or_hosted_execution_is_not_qualification(self):
        self.report["targets"][0]["device"]["physical"] = False
        with self.assertRaisesRegex(ValueError, "physical device"):
            self.check()
        self.report["targets"][0]["device"]["physical"] = True
        self.report["targets"][0]["runnerPolicy"] = "github-hosted"
        with self.assertRaisesRegex(ValueError, "runner"):
            self.check()


if __name__ == "__main__":
    unittest.main()
