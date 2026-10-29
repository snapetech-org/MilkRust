// Authored declaration/constant controls; expected RGB is (.5,.75,.25).
typedef vector<float, 3> Tone;
typedef matrix<float, 2, 3> Rows;
typedef const int Extent;
typedef Tone Palette[2];
struct Item { Tone color; float gain; };
static const uint N = (0xffffffffu + 4u);
float weights[(int)N] = {0.25, 0.5, 0.75}, gain = 2;
Tone base = {0.5, 0.75, 0.25};
static const Rows globalRows = {{1,2,3},{4,5,6}};
static const float sine = sin(0.0);
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    typedef vector<int, (1+2)> Indices;
    Extent local = (int)float(3.9);
    const uint count = uint(local) == 3u ? 3u : 1u;
    float values[count] = {0.25, 0.5, 0.75};
    float inferred[] = {1,2,3,};
    float one = 1, pair[2] = {2,3}, last = 4;
    row_major Rows rows = {{1,2,3},{4,5,6}};
    static const Rows constantRows = {{1,2,3},{4,5,6}};
    Palette colors = {{0.5,0.75,0.25}, {1,0,0}};
    Item item = {{0.5,0.75,0.25}, 1};
    vector<float> defaults = {1,2,3,4};
    matrix<float> identity = float4x4(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1);
    Indices indices = {0,1,2};
    float1 scalar = {2};
    bool ok = N == 3u && weights[2] == 0.75 && gain == 2;
    ok = ok && one == 1 && pair[1] == 3 && last == 4;
    ok = ok && rows._23 == 6 && rows[1][0] == 4;
    ok = ok && globalRows._23==6 && constantRows._21==4 && sine==0;
    ok = ok && colors[0].y == 0.75 && item.gain == 1;
    ok = ok && defaults.w == 4 && identity._33 == 1 && scalar == 2;
    float sum = 0;
    [unroll(local)] for (int i=0;i<local;i++) sum += values[indices[i]];
    ok = ok && sum == 1.5 && inferred[2] == 3;
    { const uint count=1; float inner[count]={9}; ok=ok && inner[0]==9; }
    float outer[count]={1,2,3};
    ok = ok && outer[2] == 3 && all(base == item.color);
    return ok ? float4(item.color,1) : float4(1,0,0,1);
}
