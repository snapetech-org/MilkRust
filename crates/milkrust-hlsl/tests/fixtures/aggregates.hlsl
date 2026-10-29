// Authored legacy aggregate control; expected RGB is (.75,.5,.25).
struct Lane { float2 xy; int n; };
struct Block { Lane lanes[2]; float2x2 basis; bool enabled; };
typedef float Pair[2];
Block globalBlock = {float3(1,2,3), {4, {5,6}}, float2x2(7,8,9,10), 1};
float2 pairOnce(inout int calls) { calls++; return float2(.25,.5); }
float scalarOnce(inout int calls) { calls++; return .75; }
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    int calls = 0;
    Block value = {{pairOnce(calls),7, {8,9,10}}, float4(11,12,13,14),false};
    bool ok = calls==1 && value.lanes[0].xy.x==.25 && value.lanes[0].xy.y==.5;
    ok = ok && value.lanes[0].n==7 && value.lanes[1].n==10;
    ok = ok && value.basis._12==12 && value.basis._21==13 && !value.enabled;
    float flattened[11] = {globalBlock};
    Block roundtrip = (Block)flattened;
    ok = ok && flattened[2]==3 && flattened[6]==7 && flattened[8]==9 && flattened[10]==1;
    ok = ok && roundtrip.lanes[1].n==6 && roundtrip.basis._21==9 && roundtrip.enabled;
    Block zero = (Block)0;
    Block filled = (Block)scalarOnce(calls);
    ok = ok && calls==2 && all(zero.lanes[1].xy==0) && zero.basis._22==0 && !zero.enabled;
    ok = ok && all(filled.lanes[0].xy==.75) && filled.lanes[1].n==0 && filled.basis._21==.75 && filled.enabled;
    Pair pair = (Pair)2;
    float2 packed[] = {1,2, {3,4}};
    float2 copied[2] = packed;
    float2 unpacked = (float2)pair;
    ok = ok && pair[1]==2 && all(unpacked==2) && copied[1].y==4;
    const int2 offset = {{0},0};
    ok = ok && all(offset==0);
    const int3 sizes = int3(2,3,4);
    const int3 adjusted = sizes + int3(1,1,1);
    const Block dimensions = {float3(1,2,3),4,5,6,float4(7,8,9,10),true};
    const int constants[] = {sizes,5};
    const int n = adjusted.y + constants[0] - dimensions.lanes[0].n;
    float selected[n] = {1,2,3};
    const float2x3 rows = {{1,2,3},{4,5,6}};
    const int matrixCount = (int)rows[1][1] - (int)rows._13;
    float matrixSelected[matrixCount] = {7,8};
    ok = ok && n==3 && selected[2]==3 && matrixSelected[1]==8;
    return ok ? float4(.75,.5,.25,1) : float4(1,0,0,1);
}
