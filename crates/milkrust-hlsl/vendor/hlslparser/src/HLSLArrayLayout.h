// Independently authored multidimensional HLSL -> ESSL 3.00 row layout.
#ifndef MILKRUST_HLSL_ARRAY_LAYOUT_H
#define MILKRUST_HLSL_ARRAY_LAYOUT_H
#include "HLSLTree.h"
#include <map>
#include <string>

namespace M4 {
// Run after identifier isolation. Canonical row names depend on the complete
// element type and inner dimensions, not variable names or const qualifiers.
// Copies, slices and function arguments therefore have identical GLSL types.
class HLSLArrayLayout : public HLSLTreeVisitor {
    HLSLTree& tree;
    std::map<std::string, const char*> names;
    size_t next = 0;
public:
    explicit HLSLArrayLayout(HLSLTree& value) : tree(value) {}
    void VisitType(HLSLType& type) override {
        CheckBudget();
        int dimensions[HLSLType::MaxArrayRank] = {};
        size_t total = 1;
        for (int i = 0; i < type.arrayRank; ++i) {
            if (!type.arraySizes[i] || !tree.GetExpressionValue(type.arraySizes[i], dimensions[i]) ||
                dimensions[i] < 1 || dimensions[i] > 4096 || total > 4096 / size_t(dimensions[i]))
                throw std::length_error("HLSL array storage exceeds 4096 elements or has an invalid dimension");
            total *= size_t(dimensions[i]);
        }
        if (type.arrayRank > 1 && IsSamplerType(type))
            throw std::invalid_argument("Multidimensional opaque sampler arrays require resource binding lowering");
        for (int i = type.arrayRank - 2; i >= 0; --i) {
            std::string key = std::to_string(int(type.baseType)) + ":" +
                (type.typeName ? type.typeName : "");
            for (int j = i + 1; j < type.arrayRank; ++j) key += ":" + std::to_string(dimensions[j]);
            auto found = names.find(key);
            if (found == names.end()) {
                if (names.size() >= 4096) throw std::length_error("HLSL array layout limit exceeded");
                std::string name;
                do {
                    CheckBudget();
                    name = "milkrust_array_row_" + std::to_string(next++);
                } while (tree.GetContainsString(name.c_str()));
                found = names.emplace(key, tree.AddString(name.c_str())).first;
            }
            type.arrayElementNames[i] = found->second;
        }
    }
    void VisitConstructorExpression(HLSLConstructorExpression* node) override {
        VisitType(node->type);
        HLSLTreeVisitor::VisitConstructorExpression(node);
    }
};
} // namespace M4
#endif
