"""Source regressions; execute only on compatible private ephemeral JIT runners."""
import importlib.util
import json
import unittest
from pathlib import Path

TOOLS = Path(__file__).parents[1]
spec = importlib.util.spec_from_file_location("convert", TOOLS / "convert_butterchurn.py")
convert = importlib.util.module_from_spec(spec)
spec.loader.exec_module(convert)
from butterchurn_glsl import convert_shader, ShaderConversionError


class DeclarativeConversion(unittest.TestCase):
    def test_scoped_arithmetic_and_math(self):
        self.assertEqual(convert.Equations("a.q1=Math.sin(a.time);a.zoom+=a.q1*.1;").statements(),
                         ["assign(q1,bcjsMathsin(time));", "assign(zoom,(zoom+(q1*.1)));"])

    def test_host_code_and_dynamic_objects_remain_unavailable(self):
        for source in ("fetch('https://example.com');", "a.x.constructor();", "a.x[0]=1;",
                       "new Function('x')();", "a.x=()=>1;", "a.x=Math.random();",
                       "a[a.x]=1;", "a.x=function(){};", "a.bcjsTemp1=5;", "a.X=1;a.x=2;"):
            with self.subTest(source=source), self.assertRaises(convert.Unsupported):
                convert.Equations(source).statements()

    def test_generated_js_control_flow_and_memory_are_converted_as_data(self):
        source = ("a['int']=0;for(var b=a.n=0;10>b;b++)a.gmegabuf[a.n]=0,a.n+=1;"
                  "do{a.q1+=1;if(a.q1>3)break;if(a.q1<2)continue;a.q2++;}while(a.q1<8);"
                  "a.q3=a.q1?Math.min(2,3,4):a.q2;")
        result = "\n".join(convert.Equations(source).statements())
        self.assertIn("while(", result)
        self.assertIn("gmegabuf(", result)
        self.assertIn("bcjsTruth(", result)
        self.assertIn("bcjsMathmin(bcjsMathmin(2,3),4)", result)
        self.assertNotIn("a.", result)
        self.assertNotIn("for(", result)
        self.assertNotIn("break;", result)

    def test_numeric_semantics_use_explicit_vm_operations(self):
        result = "\n".join(convert.Equations(
            "a.x=a.y%2;a.x=a.y/0.000001;a.z=a.x===a.y;"
            "a.s=a.x>>>33;a.k=~a.x;a.n=a.x^7;a.q=a.x&&++a.y;a.r=a.x||++a.y;"
        ).statements())
        for helper in ("bcjsRemainder", "bcjsDivide", "bcjsEqual", "bcjsUnsignedShiftRight",
                       "bcjsBitNot", "bcjsBitXor", "bcjsTruth"):
            self.assertIn(helper + "(", result)
        self.assertNotIn("&&", result)
        self.assertNotIn("||", result)

    def test_var_hoisting_does_not_reset_a_local_inside_a_loop(self):
        result = "\n".join(convert.Equations("var b=0;do{var c;c=b++;}while(b<3);a.x=c;").statements())
        # One invocation initialization and one explicit c=b++ assignment;
        # the bare var c never overwrites c in subsequent iterations.
        self.assertEqual(result.count("assign(bcjsLocal_63,"), 2)
        self.assertIn("assign(bcjsLocal_63,bcjsDivide(0,0))", result)
        self.assertIn("assign(x,bcjsLocal_63)", result)

    def test_memory_targets_and_postfix_updates_are_captured_once(self):
        result = convert.Equations("a.megabuf[a.x++]=a.x++;").statements()[0]
        self.assertEqual(result.count("assign(x,"), 2)
        self.assertEqual(result.count("assign(megabuf("), 1)
        self.assertIn("exec2(", result)

    def test_preserves_classic_fields_objects_and_portable_requirement(self):
        text = convert.convert({"baseVals": {"decay": .98, "wave_mode": 2},
                                "shapes": [{"baseVals": {"enabled": 1}, "frame_eqs_str": "a.x=.5;"}]}, "Author - Title")
        self.assertIn("fDecay=", text)
        self.assertIn("nWaveMode=2", text)
        self.assertIn("shape_0_per_frame1=assign(x,.5);", text)
        self.assertIn("// MILKRUST_REQUIRED_PROFILE=butterchurn-numeric-v1", text)

    def test_both_shader_stages_are_preserved(self):
        text = convert.convert({"warp": "shader_body { ret=texture(sampler_main,uv).rgb; }",
                                "comp": "shader_body { ret=mix(ret,vec3(1),.25); }"}, "shader")
        self.assertIn("PSVERSION=3", text)
        self.assertIn("warp_1=", text)
        self.assertIn("comp_1=", text)
        self.assertIn("bcglslTexture2D(sampler_main,uv).rgb", text)
        self.assertIn("bcglslMix(ret,float3(1),.25)", text)
        self.assertIn("rad=length(uv_orig-", text)

    def test_shader_matrices_keep_column_indexing_and_product_order(self):
        result = convert_shader(
            "shader_body { mat2 m=mat2(1,2,3,4); vec2 p=m*vec2(5,6);"
            "m[0].xy=vec2(8,9); mat3 n=mat3(2.0); ret=vec3(p,n[2].z); }")
        self.assertIn("float2x2 m=float2x2(1,2,3,4)", result)
        self.assertIn("bcglslMul(m,float2(5,6))", result)
        self.assertIn("float2 bcglslMul(float2x2 a,float2 b){return mul(b,a);}", result)
        self.assertIn("bcglslMatrix3x3(2.0)", result)
        self.assertIn("float3x3(a,0.0,0.0,0.0,a,0.0,0.0,0.0,a)", result)
        self.assertIn("m[0].xy", result)

    def test_glsl_comparisons_and_float_mod_are_not_hlsl_shortcuts(self):
        result = convert_shader(
            "shader_body { bool same=vec3(1)==vec3(1); bvec3 mask=lessThan(vec3(0),vec3(1));"
            "ret=mix(vec3(0),mod(vec3(-1),2.0),mask); }")
        self.assertIn("bool bcglslIsEqual(float3 a,float3 b){return all(a==b);}", result)
        self.assertIn("return a-b*floor(a/b);", result)
        self.assertIn("bcglslLessThan(float3(0),float3(1))", result)
        self.assertIn("bool3 t){return t?b:a;}", result)

    def test_samplers_and_unsupported_shader_input(self):
        result = convert_shader(
            "uniform sampler3D sampler_noisevol_hq;"
            "shader_body {ret=textureLod(sampler_noisevol_hq,vec3(.5),0.0).rgb;}")
        self.assertIn("bcglslTexture3DLod(sampler_noisevol_hq,float3(.5),0.0)", result)
        for source in ("#include \"outside.fx\"\nshader_body {}", "void main(){}",
                       "shader_body{} shader_body{}", "uniform float unknown;shader_body{}"):
            with self.subTest(source=source), self.assertRaises(ShaderConversionError):
                convert_shader(source)

    def test_sampler_parameters_keep_their_lexical_dimensions(self):
        result = convert_shader(
            "vec4 planar(sampler2D s,vec2 uv){return texture(s,uv);}"
            "vec4 volume(sampler3D s,vec3 uv){return texture(s,uv);}"
            "shader_body {ret=planar(sampler_main,uv).rgb+volume(sampler_noisevol_lq,vec3(uv,0)).rgb;}")
        self.assertIn("return bcglslTexture2D(s,uv)", result)
        self.assertIn("return bcglslTexture3D(s,uv)", result)
        with self.assertRaises(ShaderConversionError):
            convert_shader("shader_body {float sampler_noisevol_lq=0;ret=texture(sampler_noisevol_lq,vec3(0)).rgb;}")

    def test_limits_fail_before_recursion_or_unbounded_output(self):
        for source in ("a.x=" + "(" * 80 + "1" + ")" * 80 + ";", "a.x=1;" * 12000):
            with self.assertRaises(convert.Unsupported):
                convert.Equations(source).statements()
        with self.assertRaises(ShaderConversionError):
            convert_shader("shader_body { ret=" + "(" * 80 + "vec3(1)" + ")" * 80 + ";}")

    def test_real_generated_preset_keeps_equations_and_shaders(self):
        path = TOOLS.parent / "content/third-party/butterchurn-presets-2.4.7/presets/converted/!!!---flexi + amandio c - organic12-3d-2.json"
        result = convert.convert(json.loads(path.read_text()), path.stem)
        self.assertIn("warp_1=", result)
        self.assertIn("comp_1=", result)
        self.assertIn("while(", result)
        self.assertIn("megabuf(", result)


if __name__ == "__main__":
    unittest.main()
