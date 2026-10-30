"""Source regression cases; run on an authorized private JIT runner."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from validate_pack_dependencies import validate_pack_dependencies


class DependencySchemaTests(unittest.TestCase):
    def test_texture_pack_requires_real_inventory(self):
        base = {"id": "textures", "version": "1", "kind": "textures", "presets": []}
        with self.assertRaises(ValueError):
            validate_pack_dependencies(base)
        self.assertEqual(validate_pack_dependencies({**base, "textures": [{"file": "textures/a.png"}]}), "textures")

    def test_dependency_graph_is_pinned_and_acyclic(self):
        dependency = {"id": "textures", "version": "1", "integritySha256": "a" * 64}
        self.assertEqual(validate_pack_dependencies({"dependencies": [dependency]}), "presets")
        for bad in [
            {"dependencies": [dependency, dependency]},
            {"id": "textures", "version": "1", "dependencies": [dependency]},
            {"dependencies": [{**dependency, "version": "moving.previous"}]},
            {"dependencies": [{**dependency, "integritySha256": "latest"}]},
            {"kind": "textures", "presets": [], "textures": [{"file": "a.png"}], "dependencies": [dependency]},
        ]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                validate_pack_dependencies(bad)

    def test_engine_requirements_are_explicit_and_versioned(self):
        preset = {"requiredEngine": "milkrust", "requiredProfile": "butterchurn-numeric-v1"}
        self.assertEqual(validate_pack_dependencies({"presets": [preset]}), "presets")
        for bad in [{"requiredEngine": "future"}, {"requiredProfile": "butterchurn-numeric-v1"},
                    {**preset, "requiredProfile": "future"}]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                validate_pack_dependencies({"presets": [bad]})


if __name__ == "__main__":
    unittest.main()
