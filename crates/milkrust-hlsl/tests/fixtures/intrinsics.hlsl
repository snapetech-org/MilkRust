// Authored legacy operator/intrinsic controls; expected RGB is (.75,.25,.5).
float mad(float x) { return x+1; }
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    bool ok=abs(rcp(4)-0.25)<1e-7 && abs(rsqrt(4)-0.5)<1e-7;
    ok=ok && mad(2,3,4)==10 && mad(1)==2;
    ok=ok && all(bool3(true,true,true)) && !any(float3(0,0,0));
    ok=ok && all(float2x3(1)) && !all(float2x2(1,1,1,0));
    ok=ok && round(2.5)==2 && round(3.5)==4 && trunc(-1.75)==-1;
    float2 integral;
    float2 fraction=modf(float2(-1.25,2.75),integral);
    ok=ok && integral.x==-1 && integral.y==2 && fraction.x==-0.25 && fraction.y==0.75;
    float2 exponent;
    float2 mantissa=frexp(float2(-8,0),exponent);
    ok=ok && mantissa.x==-0.5 && exponent.x==4 && mantissa.y==0 && exponent.y==0;
    ok=ok && ldexp(mantissa.x,exponent.x)==-8;
    float2x2 angles=0, sine, cosine;
    sincos(angles,sine,cosine);
    ok=ok && all(cosine) && !any(sine);
    float2 s,c;
    sincos(float2(0,0),s,c);
    ok=ok && all(c==1) && all(s==0);
    ok=ok && all(isfinite(float3(1,2,3))) && !any(isnan(float2(0,1)));
    ok=ok && asuint(1.0)==0x3f800000u && asfloat(0x3f800000u)==1.0;
    uint bits=0x80000000u;
    bits >>= 30u;
    bits <<= 2u;
    bits |= 3u;
    bits &= 10u;
    bits ^= 1u;
    bits %= 4u;
    ok=ok && bits==3u && (17%5)==2 && (-5.5%2.0)==-1.5;
    float remainder=-5.5;
    remainder%=2.0;
    ok=ok && remainder==-1.5;
    ok=ok && abs(((int)1.9+0.9)-1.9)<1e-6;
    ok=ok && (false ? 1 : true ? 2 : 3)==2;
    ok=ok && (1<<2+1)==8 && (8>>1+1)==2;
    ok=ok && 1e-8>0 && 1.00000012>1;
    float2x3 values=round(float2x3(2.5,3.5,0.5,4.5,5.5,6.5));
    ok=ok && values._11==2 && values._12==4 && values._23==6;
    return ok ? float4(0.75,0.25,0.5,1) : float4(1,0,0,1);
}
