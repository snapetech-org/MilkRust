// Authored numeric controls. A passing shader returns (0.25, 0.5, 0.75, 1).
// This is a language-semantics probe, not a MilkDrop reference capture.
void pair(out float a, out float b) { a=5; b=6; }
void shift(inout float2 value) { value+=float2(0.5,1.5); }
void increment(inout float value) { value+=1; }
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    float2x2 m=float2x2(1,2,3,4);
    float2x2 component=m*m;
    bool ok=component._12==4 && component._21==9;
    float2 mv=mul(m,float2(1,2));
    ok=ok && mv.x==5 && mv.y==11;
    float2x2 product=mul(m,m);
    ok=ok && product._12==10 && product._21==15;
    ok=ok && mul(float2(2,3),float2(4,5))==23;
    float2x2 splat=2;
    ok=ok && splat._12==2 && splat._21==2;
    float2 selected=m._m01_m10;
    m._m01_m10=selected.yx;
    ok=ok && m._12==3 && m._21==2;
    m[0].yx=float2(7,8);
    int row=0;
    m[row++][1]+=2;
    float old=m[1][0]++;
    ok=ok && row==1 && m._11==8 && m._12==9 && old==2 && m._21==3;
    pair(m._11,m._12);
    ok=ok && m._11==5 && m._12==6;
    row=0;
    increment(m[row++][1]);
    ok=ok && row==1 && m._12==7;
    int2 converted=int2(2,3);
    shift(converted);
    ok=ok && converted.x==2 && converted.y==4;
    bool2 less=int3(1,4,9)<float2(2,3);
    bool2 logic=less && !bool2(false,false);
    int2 choice=logic ? int2(8,9) : int2(2,3);
    ok=ok && choice.x==8 && choice.y==3 && (~1)==-2;
    float4x3 rectangle=float4x3(1,2,3,4,5,6,7,8,9,10,11,12);
    rectangle[3].yz=float2(13,14);
    ok=ok && rectangle._m30==10 && rectangle._m31==13 && rectangle._m32==14;
    float3x4 transposed=transpose(rectangle);
    ok=ok && transposed._m03==10 && transposed._m13==13;
    float2x3 left=float2x3(1,2,3,4,5,6);
    float3x2 right=float3x2(1,0,0,1,1,1);
    float2x2 rectangularProduct=mul(left,right);
    ok=ok && rectangularProduct._11==4 && rectangularProduct._12==5 && rectangularProduct._21==10 && rectangularProduct._22==11;
    return ok ? float4(0.25,0.5,0.75,1) : float4(1,0,0,1);
}
