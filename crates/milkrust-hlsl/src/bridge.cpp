// MilkRust's only C++ entry point. No C++ object or exception crosses this ABI.
#include "Engine.h"
#include "HLSLParser.h"
#include "HLSLTree.h"
#include "GLSLGenerator.h"
#include <cstring>
#include <string>
#include <exception>

class ValidateArrayBounds : public M4::HLSLTreeVisitor {
    M4::HLSLTree& tree;
public:
    explicit ValidateArrayBounds(M4::HLSLTree& value) : tree(value) {}
    void VisitType(M4::HLSLType& type) override {
        M4::CheckBudget();
        if (type.array) {
            int count = 0;
            if (!type.arraySize || !tree.GetExpressionValue(type.arraySize, count) || count < 1 || count > 4096)
                throw std::length_error("HLSL array must have a constant bound between 1 and 4096");
        }
    }
};

static void copy_error(char* target, size_t capacity, const char* message) {
    if (!target || capacity == 0) return;
    const size_t size = std::strlen(message);
    const size_t copied = size < capacity - 1 ? size : capacity - 1;
    std::memcpy(target, message, copied);
    target[copied] = 0;
}

extern "C" int milkrust_hlsl_compile(const char* source, size_t size,
    const char* entry, char* output, size_t capacity, char* error, size_t error_capacity) noexcept {
    if (output && capacity) output[0] = 0;
    if (error && error_capacity) error[0] = 0;
    if (!source || !entry || !output || size > 256 * 1024 || capacity == 0) {
        copy_error(error, error_capacity, "Invalid compiler input or source limit");
        return -1;
    }
    try {
        M4::ResetLimits();
        // Tokenizer's numeric conversion requires a NUL terminator.
        const std::string terminated(source, size);
        M4::Allocator allocator;
        M4::HLSLTree tree(&allocator);
        M4::HLSLParser parser(&allocator, "preset.hlsl", terminated.c_str(), size);
        if (!parser.Parse(&tree)) {
            copy_error(error, error_capacity, M4::Diagnostics());
            return -1;
        }
        ValidateArrayBounds bounds(tree);
        bounds.VisitRoot(tree.GetRoot());
        M4::GLSLGenerator generator;
        const M4::GLSLGenerator::Options options;
        if (!generator.Generate(&tree, M4::GLSLGenerator::Target_FragmentShader,
            M4::GLSLGenerator::Version_300_ES, entry, options)) {
            copy_error(error, error_capacity, M4::Diagnostics());
            return -1;
        }
        const char* result = generator.GetResult();
        const size_t length = std::strlen(result);
        if (length >= capacity) throw std::length_error("GLSL output limit exceeded");
        std::memcpy(output, result, length + 1);
        return static_cast<int>(length);
    } catch (const std::exception& exception) {
        copy_error(error, error_capacity, exception.what());
    } catch (...) {
        copy_error(error, error_capacity, "HLSL compiler failed");
    }
    return -1;
}
