// MilkRust numeric shape rules shared by the typed parser and GLSL emitter.
#ifndef MILKRUST_HLSL_NUMERIC_H
#define MILKRUST_HLSL_NUMERIC_H
#include "HLSLTree.h"
#include <algorithm>
namespace M4 {
inline int MatrixRows(HLSLBaseType type) {
    if (type == HLSLBaseType_Float2x2) return 2;
    if (type == HLSLBaseType_Float2x3) return 2;
    if (type == HLSLBaseType_Float2x4) return 2;
    if (type == HLSLBaseType_Float3x2) return 3;
    if (type == HLSLBaseType_Float3x3) return 3;
    if (type == HLSLBaseType_Float3x4) return 3;
    if (type == HLSLBaseType_Float4x2) return 4;
    if (type == HLSLBaseType_Float4x3) return 4;
    if (type == HLSLBaseType_Float4x4) return 4;
    if (type == HLSLBaseType_Half2x2) return 2;
    if (type == HLSLBaseType_Half2x3) return 2;
    if (type == HLSLBaseType_Half2x4) return 2;
    if (type == HLSLBaseType_Half3x2) return 3;
    if (type == HLSLBaseType_Half3x3) return 3;
    if (type == HLSLBaseType_Half3x4) return 3;
    if (type == HLSLBaseType_Half4x2) return 4;
    if (type == HLSLBaseType_Half4x3) return 4;
    if (type == HLSLBaseType_Half4x4) return 4;
    return 0;
}
inline int NumericWidth(HLSLBaseType type) {
    if (type == HLSLBaseType_Float2x2) return 2;
    if (type == HLSLBaseType_Float2x3) return 3;
    if (type == HLSLBaseType_Float2x4) return 4;
    if (type == HLSLBaseType_Float3x2) return 2;
    if (type == HLSLBaseType_Float3x3) return 3;
    if (type == HLSLBaseType_Float3x4) return 4;
    if (type == HLSLBaseType_Float4x2) return 2;
    if (type == HLSLBaseType_Float4x3) return 3;
    if (type == HLSLBaseType_Float4x4) return 4;
    if (type == HLSLBaseType_Half2x2) return 2;
    if (type == HLSLBaseType_Half2x3) return 3;
    if (type == HLSLBaseType_Half2x4) return 4;
    if (type == HLSLBaseType_Half3x2) return 2;
    if (type == HLSLBaseType_Half3x3) return 3;
    if (type == HLSLBaseType_Half3x4) return 4;
    if (type == HLSLBaseType_Half4x2) return 2;
    if (type == HLSLBaseType_Half4x3) return 3;
    if (type == HLSLBaseType_Half4x4) return 4;
    if (IsScalarType(type)) return 1;
    if (IsVectorType(type)) return int(type) - int(ScalarBaseType[type]) + 1;
    return 0;
}

inline HLSLBaseType NumericShape(HLSLBaseType scalar, int rows, int width) {
    if (!rows) return width >= 1 && width <= 4 ? HLSLBaseType(int(scalar) + width - 1) : HLSLBaseType_Unknown;
    const HLSLBaseType types[] = {
        HLSLBaseType_Float2x2,
        HLSLBaseType_Float2x3,
        HLSLBaseType_Float2x4,
        HLSLBaseType_Float3x2,
        HLSLBaseType_Float3x3,
        HLSLBaseType_Float3x4,
        HLSLBaseType_Float4x2,
        HLSLBaseType_Float4x3,
        HLSLBaseType_Float4x4,
        HLSLBaseType_Half2x2,
        HLSLBaseType_Half2x3,
        HLSLBaseType_Half2x4,
        HLSLBaseType_Half3x2,
        HLSLBaseType_Half3x3,
        HLSLBaseType_Half3x4,
        HLSLBaseType_Half4x2,
        HLSLBaseType_Half4x3,
        HLSLBaseType_Half4x4,
    };
    for (auto type : types) if (ScalarBaseType[type] == scalar && MatrixRows(type) == rows && NumericWidth(type) == width) return type;
    return HLSLBaseType_Unknown;
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
