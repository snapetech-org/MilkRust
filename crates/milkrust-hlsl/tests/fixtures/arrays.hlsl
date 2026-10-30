// Authored multidimensional-array control; expected RGB is (.25,.75,.5).
typedef float Row[3];
typedef Row Grid[2];
struct Cell { int2x2 matrix; float tail; };
struct Volume { Cell cells[2][2]; float weights[2][3]; };
Grid globals = {{1,2,3},{4,5,6}};
Volume emptyVolume;
Grid copyGrid(Grid grid) { return grid; }
void replaceRow(inout Row row) { row[0]+=10; row[2]+=20; }
float sumRow(Row row) { return row[0]+row[1]+row[2]; }
float4 main(float2 coord:TEXCOORD0):COLOR0 {
    Grid grid = copyGrid(globals);
    int index=0;
    replaceRow(grid[index++]);
    bool ok = index==1 && grid[0][0]==11 && grid[0][2]==23;
    ok = ok && sumRow(grid[1])==15 && globals[0][0]==1;
    float inferred[][3] = {{1,2,3},{4,5,6}};
    Row copied = inferred[1];
    ok = ok && copied[0]==4 && copied[2]==6;
    int first=0, second=1;
    grid[first++][second++] += 30;
    ok = ok && first==1 && second==2 && grid[0][1]==32;
    float flattened[6] = {grid};
    Grid restored = (Grid)flattened;
    Grid filled = (Grid)7;
    ok = ok && restored[0][2]==23 && restored[1][0]==4 && filled[1][2]==7;
    float cube[2][1][3] = {{{1,2,3}},{{4,5,6}}};
    ok = ok && cube[1][0][2]==6;
    Volume volume = (Volume)0;
    volume.cells[1][0].matrix = int2x2(1,2,3,4);
    volume.cells[1][0].tail = 8;
    volume.weights[0][1] = 9;
    Volume twin = volume;
    ok = ok && twin.cells[1][0].matrix._21==3 && twin.cells[1][0].tail==8;
    ok = ok && twin.weights[0][1]==9 && emptyVolume.cells[1][1].matrix._22==0;
    int iterations=0;
    for (float a[1][2]={{1,2}}, b[1][3]={{3,4,5}}; iterations<1; iterations++) {
        ok = ok && a[0][1]==2 && b[0][2]==5;
    }
    const int bounds[2][2]={{2,3},{4,5}};
    float bounded[bounds[0][0]][bounds[0][1]] = {{1,2,3},{4,5,6}};
    ok = ok && bounded[1][2]==6;
    return ok ? float4(.25,.75,.5,1) : float4(1,0,0,1);
}
