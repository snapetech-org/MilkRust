// Independently authored bounded legacy HLSL aggregate layout.
#ifndef MILKRUST_HLSL_AGGREGATES_H
#define MILKRUST_HLSL_AGGREGATES_H
#include "HLSLTree.h"
#include "HLSLNumeric.h"
#include <vector>

namespace M4 {
inline int AggregateArrayExtent(HLSLTree& tree, const HLSLType& type) {
    int count = 0;
    if (!type.arrayRank || !type.arraySizes[0] || !tree.GetExpressionValue(type.arraySizes[0], count) || count < 1 || count > 4096)
        throw std::length_error("HLSL aggregate array bound must be between 1 and 4096");
    return count;
}

// The ordering is HLSL row-major for matrices and declaration order for fields.
// A separate component budget prevents nested structs/arrays multiplying the
// permitted declaration bounds into an unbounded generated initializer.
inline void AggregateScalarTypes(HLSLTree& tree, const HLSLType& type,
    std::vector<HLSLBaseType>& out) {
    DepthGuard depth;
    if (type.array) {
        const int count = AggregateArrayExtent(tree, type);
        HLSLType element = type; element.DropArrayDimension();
        for (int i = 0; i < count; ++i) AggregateScalarTypes(tree, element, out);
    } else if (type.baseType == HLSLBaseType_UserDefined) {
        auto* structure = tree.FindGlobalStruct(type.typeName);
        if (!structure || !structure->field) throw std::invalid_argument("Invalid HLSL aggregate structure");
        for (auto* field = structure->field; field; field = field->nextField)
            AggregateScalarTypes(tree, field->type, out);
    } else {
        const int width = NumericWidth(type.baseType), rows = MatrixRows(type.baseType);
        if (!width) throw std::invalid_argument("HLSL aggregate requires numeric fields");
        const size_t count = size_t(width * (rows ? rows : 1));
        if (out.size() + count > 4096) throw std::length_error("HLSL aggregate component limit exceeded");
        out.insert(out.end(), count, ScalarBaseType[type.baseType]);
    }
}
} // namespace M4
#endif
