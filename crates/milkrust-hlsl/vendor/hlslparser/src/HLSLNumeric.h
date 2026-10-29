// MilkRust numeric shape rules shared by the typed parser and GLSL emitter.
#ifndef MILKRUST_HLSL_NUMERIC_H
#define MILKRUST_HLSL_NUMERIC_H
#include "HLSLTree.h"
#include <algorithm>
namespace M4 {
struct HLSLMatrixShape { HLSLBaseType type; int rows; int columns; };
inline const HLSLMatrixShape* MatrixShapes(size_t& count) {
    static const HLSLMatrixShape shapes[] = {
#define MR(s, r, c) { HLSLBaseType_##s##r##x##c, r, c }
        MR(Float,1,1), MR(Float,1,2), MR(Float,1,3), MR(Float,1,4),
        MR(Float,2,1), MR(Float,2,2), MR(Float,2,3), MR(Float,2,4),
        MR(Float,3,1), MR(Float,3,2), MR(Float,3,3), MR(Float,3,4),
        MR(Float,4,1), MR(Float,4,2), MR(Float,4,3), MR(Float,4,4),
        MR(Half,1,1), MR(Half,1,2), MR(Half,1,3), MR(Half,1,4),
        MR(Half,2,1), MR(Half,2,2), MR(Half,2,3), MR(Half,2,4),
        MR(Half,3,1), MR(Half,3,2), MR(Half,3,3), MR(Half,3,4),
        MR(Half,4,1), MR(Half,4,2), MR(Half,4,3), MR(Half,4,4),
        MR(Bool,1,1), MR(Bool,1,2), MR(Bool,1,3), MR(Bool,1,4),
        MR(Bool,2,1), MR(Bool,2,2), MR(Bool,2,3), MR(Bool,2,4),
        MR(Bool,3,1), MR(Bool,3,2), MR(Bool,3,3), MR(Bool,3,4),
        MR(Bool,4,1), MR(Bool,4,2), MR(Bool,4,3), MR(Bool,4,4),
        MR(Int,1,1), MR(Int,1,2), MR(Int,1,3), MR(Int,1,4),
        MR(Int,2,1), MR(Int,2,2), MR(Int,2,3), MR(Int,2,4),
        MR(Int,3,1), MR(Int,3,2), MR(Int,3,3), MR(Int,3,4),
        MR(Int,4,1), MR(Int,4,2), MR(Int,4,3), MR(Int,4,4),
        MR(Uint,1,1), MR(Uint,1,2), MR(Uint,1,3), MR(Uint,1,4),
        MR(Uint,2,1), MR(Uint,2,2), MR(Uint,2,3), MR(Uint,2,4),
        MR(Uint,3,1), MR(Uint,3,2), MR(Uint,3,3), MR(Uint,3,4),
        MR(Uint,4,1), MR(Uint,4,2), MR(Uint,4,3), MR(Uint,4,4),
#undef MR
    };
    count = sizeof(shapes) / sizeof(shapes[0]);
    return shapes;
}
inline int MatrixRows(HLSLBaseType type) {
    size_t count = 0; const auto* shapes = MatrixShapes(count);
    for (size_t i = 0; i < count; ++i) if (shapes[i].type == type) return shapes[i].rows;
    return 0;
}
inline int MatrixColumns(HLSLBaseType type) {
    size_t count = 0; const auto* shapes = MatrixShapes(count);
    for (size_t i = 0; i < count; ++i) if (shapes[i].type == type) return shapes[i].columns;
    return 0;
}
inline int NumericWidth(HLSLBaseType type) {
    if (IsMatrixType(type)) return MatrixColumns(type);
    if (IsScalarType(type)) return 1;
    if (IsVectorType(type)) return int(type) - int(ScalarBaseType[type]) + 1;
    return 0;
}
inline HLSLBaseType NumericShape(HLSLBaseType scalar, int rows, int width) {
    if (!rows) return width >= 1 && width <= 4 ? HLSLBaseType(int(scalar) + width - 1) : HLSLBaseType_Unknown;
    size_t count = 0; const auto* shapes = MatrixShapes(count);
    for (size_t i = 0; i < count; ++i)
        if (ScalarBaseType[shapes[i].type] == scalar && shapes[i].rows == rows && shapes[i].columns == width)
            return shapes[i].type;
    return HLSLBaseType_Unknown;
}
inline bool IsCustomMatrixType(HLSLBaseType type) {
    if (!IsMatrixType(type)) return false;
    const auto scalar = ScalarBaseType[type];
    return (scalar == HLSLBaseType_Bool || scalar == HLSLBaseType_Int || scalar == HLSLBaseType_Uint) &&
        MatrixRows(type) > 1 && MatrixColumns(type) > 1;
}
inline HLSLBaseType CommonNumericType(HLSLBaseType lhs, HLSLBaseType rhs) {
    const int a = NumericWidth(lhs), b = NumericWidth(rhs);
    if (!a || !b) return HLSLBaseType_Unknown;
    const int ar = MatrixRows(lhs), br = MatrixRows(rhs);
    if ((ar && !br && b > 1) || (br && !ar && a > 1)) return HLSLBaseType_Unknown;
    const int rows = ar && br ? std::min(ar, br) : std::max(ar, br);
    const int width = IsScalarType(lhs) ? b : IsScalarType(rhs) ? a : std::min(a, b);
    // Usual arithmetic conversion: float, half, unsigned, signed, bool.
    HLSLBaseType scalar = HLSLBaseType_Bool;
    const HLSLBaseType order[] = {HLSLBaseType_Float, HLSLBaseType_Half, HLSLBaseType_Uint, HLSLBaseType_Int, HLSLBaseType_Bool};
    for (auto candidate : order) if (ScalarBaseType[lhs] == candidate || ScalarBaseType[rhs] == candidate) { scalar = candidate; break; }
    return NumericShape(scalar == HLSLBaseType_Bool ? HLSLBaseType_Int : scalar, rows, width);
}
}
#endif
