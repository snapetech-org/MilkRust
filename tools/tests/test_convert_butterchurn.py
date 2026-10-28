"""Source regression cases; execution is reserved for private JIT runners."""
import importlib.util
import unittest
from pathlib import Path
spec = importlib.util.spec_from_file_location("convert", Path(__file__).parents[1] / "convert_butterchurn.py")
convert = importlib.util.module_from_spec(spec)
spec.loader.exec_module(convert)

class RestrictedConversion(unittest.TestCase):
    def test_scoped_arithmetic_and_math(self):
        self.assertEqual(convert.Equations("a.q1=Math.sin(a.time);a.zoom+=a.q1*.1;").statements(),
                         ["q1=sin(time);", "zoom+=(q1*.1);"])

    def test_rejects_executable_language_features(self):
        for source in ("fetch('https://example.com');", "a.x.constructor();", "a.x[0]=1;",
                       "while(1){}", "new Function('x')();", "a.x=()=>1;", "a.x=a.y%2;",
                       "a.x=Math.random();", "a.x=a.y?1:2;", "a.x=1||2;"):
            with self.subTest(source=source), self.assertRaises(convert.Unsupported):
                convert.Equations(source).statements()

    def test_preserves_classic_field_names_and_object_equations(self):
        text = convert.convert({"baseVals": {"decay": .98, "wave_mode": 2},
                                "shapes": [{"baseVals": {"enabled": 1}, "frame_eqs_str": "a.x=.5;"}]}, "Author - Title")
        self.assertIn("fDecay=", text)
        self.assertIn("nWaveMode=2", text)
        self.assertIn("shape_0_per_frame1=x=.5;", text)

    def test_shaders_cannot_disappear_during_conversion(self):
        with self.assertRaises(convert.Unsupported):
            convert.convert({"warp": "shader_body { ret=vec3(1); }"}, "shader")

if __name__ == "__main__": unittest.main()
