// Authored GLSL identifier-collision control; expected RGB is (.25,.75,.5).
struct input { float filter; float3 sample; };
float mix(float input) { return input+1; }
float sin(float x, float y) { return x+y; }
float3 result(input value) { return value.sample*value.filter; }
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    input layout={2,{0.125,0.375,0.25}};
    float gl_value=1, webgl_color=2, private__name=3;
    float mod=4, fract=5, dFdx=6;
    float frac=0.25, tex2D=0.5, clip=0.75;
    bool flag=true;
    bool ok=mix(2)==3 && sin(1,2)==3 && sin(0)==0;
    ok=ok && gl_value+webgl_color+private__name==6;
    ok=ok && mod+fract+dFdx==15;
    ok=ok && frac+tex2D+clip==1.5 && all(flag.xxx);
    float3 color=result(layout);
    return ok ? float4(color,1) : float4(1,0,0,1);
}
