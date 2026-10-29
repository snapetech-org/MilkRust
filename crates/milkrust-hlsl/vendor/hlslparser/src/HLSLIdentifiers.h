// MilkRust: rename HLSL user identifiers which cannot safely appear in GLSL ES.
// Keywords/identifier namespaces: GLSL ES 3.00 sections 3.8 and 3.9. Built-ins
// are also protected so a user overload does not redefine a GLSL intrinsic.
#ifndef MILKRUST_HLSL_IDENTIFIERS_H
#define MILKRUST_HLSL_IDENTIFIERS_H
#include "HLSLTree.h"
#include <cstring>
#include <map>
#include <set>
#include <sstream>
#include <string>
namespace M4 {
class GLSLIdentifiers : public HLSLTreeVisitor {
    HLSLTree& tree;
    std::set<std::string> reserved;
    std::map<std::string, const char*> names;
    unsigned next = 0;
public:
    explicit GLSLIdentifiers(HLSLTree& value) : tree(value) {
        std::istringstream words(
            "const uniform layout centroid flat smooth break continue do for while switch case default if else "
            "in out inout float int void bool true false invariant discard return struct uint lowp mediump highp precision "
            "mat2 mat3 mat4 mat2x2 mat2x3 mat2x4 mat3x2 mat3x3 mat3x4 mat4x2 mat4x3 mat4x4 "
            "vec2 vec3 vec4 ivec2 ivec3 ivec4 bvec2 bvec3 bvec4 uvec2 uvec3 uvec4 "
            "sampler2D sampler3D samplerCube sampler2DShadow samplerCubeShadow sampler2DArray sampler2DArrayShadow "
            "isampler2D isampler3D isamplerCube isampler2DArray usampler2D usampler3D usamplerCube usampler2DArray "
            "attribute varying coherent volatile restrict readonly writeonly resource atomic_uint noperspective patch sample subroutine "
            "common partition active asm class union enum typedef template this goto inline noinline public static extern external interface "
            "long short double half fixed unsigned superp input output hvec2 hvec3 hvec4 dvec2 dvec3 dvec4 fvec2 fvec3 fvec4 "
            "sampler3DRect filter image1D image2D image3D imageCube iimage1D iimage2D iimage3D iimageCube "
            "uimage1D uimage2D uimage3D uimageCube image1DArray image2DArray iimage1DArray iimage2DArray uimage1DArray uimage2DArray "
            "imageBuffer iimageBuffer uimageBuffer sampler1D sampler1DShadow sampler1DArray sampler1DArrayShadow "
            "isampler1D isampler1DArray usampler1D usampler1DArray sampler2DRect sampler2DRectShadow isampler2DRect usampler2DRect "
            "samplerBuffer isamplerBuffer usamplerBuffer sampler2DMS isampler2DMS usampler2DMS sampler2DMSArray isampler2DMSArray usampler2DMSArray "
            "sizeof cast namespace using "
            "radians degrees sin cos tan asin acos atan sinh cosh tanh asinh acosh atanh pow exp log exp2 log2 sqrt inversesqrt "
            "abs sign floor trunc round roundEven ceil fract mod modf min max clamp mix step smoothstep isnan isinf "
            "floatBitsToInt floatBitsToUint intBitsToFloat uintBitsToFloat packSnorm2x16 unpackSnorm2x16 packUnorm2x16 unpackUnorm2x16 "
            "packHalf2x16 unpackHalf2x16 length distance dot cross normalize faceforward reflect refract matrixCompMult outerProduct transpose determinant inverse "
            "lessThan lessThanEqual greaterThan greaterThanEqual equal notEqual any all not "
            "textureSize texture textureProj textureLod textureOffset texelFetch texelFetchOffset textureProjOffset textureLodOffset "
            "textureProjLod textureProjLodOffset textureGrad textureGradOffset textureProjGrad textureProjGradOffset dFdx dFdy fwidth");
        std::string word;
        while (words >> word) reserved.insert(word);
    }
    const char* Name(const char* original) {
        CheckBudget();
        if (!original) return original;
        const auto found = names.find(original);
        if (found != names.end()) return found->second;
        if (!reserved.count(original) && std::strncmp(original, "gl_", 3) != 0 &&
            std::strncmp(original, "webgl_", 6) != 0 && std::strncmp(original, "_webgl_", 7) != 0 &&
            std::strncmp(original, "milkrust_matrix_", 16) != 0 &&
            !std::strstr(original, "__")) return original;
        if (names.size() >= 8192) throw std::length_error("HLSL renamed identifier limit exceeded");
        std::string replacement;
        do {
            CheckBudget();
            replacement = "milkrust_identifier_" + std::to_string(next++);
        } while (tree.GetContainsString(replacement.c_str()));
        const char* value = tree.AddString(replacement.c_str());
        names[original] = value;
        return value;
    }
    void VisitType(HLSLType& type) override {
        DepthGuard depth;
        if (type.baseType == HLSLBaseType_UserDefined) type.typeName = Name(type.typeName);
        for (int i = 0; i < type.arrayRank; ++i)
            if (type.arraySizes[i]) VisitExpression(type.arraySizes[i]);
    }
    void VisitDeclaration(HLSLDeclaration* node) override {
        node->name = Name(node->name); HLSLTreeVisitor::VisitDeclaration(node);
    }
    void VisitStruct(HLSLStruct* node) override {
        node->name = Name(node->name); HLSLTreeVisitor::VisitStruct(node);
    }
    void VisitStructField(HLSLStructField* node) override {
        node->name = Name(node->name); HLSLTreeVisitor::VisitStructField(node);
    }
    void VisitArgument(HLSLArgument* node) override {
        node->name = Name(node->name); HLSLTreeVisitor::VisitArgument(node);
    }
    void VisitFunction(HLSLFunction* node) override {
        if (!node->intrinsic) node->name = Name(node->name);
        HLSLTreeVisitor::VisitFunction(node);
    }
    void VisitIdentifierExpression(HLSLIdentifierExpression* node) override {
        node->name = Name(node->name);
    }
    void VisitConstructorExpression(HLSLConstructorExpression* node) override {
        VisitType(node->type); HLSLTreeVisitor::VisitConstructorExpression(node);
    }
    void VisitMemberAccess(HLSLMemberAccess* node) override {
        if (!node->swizzle) node->field = Name(node->field);
        HLSLTreeVisitor::VisitMemberAccess(node);
    }
};
}
#endif
