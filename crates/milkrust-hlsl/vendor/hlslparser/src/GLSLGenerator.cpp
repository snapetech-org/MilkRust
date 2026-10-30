//=============================================================================
//
// Render/GLSLGenerator.cpp
//
// Created by Max McGuire (max@unknownworlds.com)
// Copyright (c) 2013, Unknown Worlds Entertainment, Inc.
//
//=============================================================================

#include "GLSLGenerator.h"
#include "HLSLParser.h"
#include "HLSLTree.h"
#include "HLSLNumeric.h"
#include "HLSLAggregates.h"

//#include "Engine/String.h"
//#include "Engine/Log.h"
#include "Engine.h"

#include <stdarg.h>
#include <string.h>

namespace M4
{

static const HLSLType kFloatType(HLSLBaseType_Float);
static const HLSLType kUintType(HLSLBaseType_Uint);
static const HLSLType kIntType(HLSLBaseType_Int);
static const HLSLType kBoolType(HLSLBaseType_Bool);

// These are reserved words in GLSL that aren't reserved in HLSL.
const char* GLSLGenerator::s_reservedWord[] =
    {
        "output",
        "input",
        "mod",
        "mix",
        "fract",
        "dFdx",
        "dFdy",
    };

static const char* GetTypeName(const HLSLType& type)
{
    if (type.arrayRank > 1) {
        if (!type.arrayElementNames[0]) throw std::invalid_argument("Missing HLSL array row layout");
        return type.arrayElementNames[0];
    }
    if (IsMatrixType(type.baseType)) {
        const auto scalar = ScalarBaseType[type.baseType];
        const int rows = MatrixRows(type.baseType), columns = NumericWidth(type.baseType);
        if (rows == 1 || columns == 1)
            return GetTypeName(HLSLType(NumericShape(scalar, 0, std::max(rows, columns))));
        if (scalar == HLSLBaseType_Bool || scalar == HLSLBaseType_Int || scalar == HLSLBaseType_Uint) {
            static thread_local char customType[64];
            const char* scalarName = scalar == HLSLBaseType_Bool ? "bool" : scalar == HLSLBaseType_Int ? "int" : "uint";
            String_Printf(customType, sizeof(customType), "milkrust_matrix_%s_%dx%d", scalarName, rows, columns);
            return customType;
        }
    }
    switch (type.baseType)
    {
    case HLSLBaseType_Void:         return "void";
    case HLSLBaseType_Float:        return "float";
    case HLSLBaseType_Float2:       return "vec2";
    case HLSLBaseType_Float3:       return "vec3";
    case HLSLBaseType_Float4:       return "vec4";
	case HLSLBaseType_Float2x2:     return "mat2";
    case HLSLBaseType_Float3x3:     return "mat3";
    case HLSLBaseType_Float4x4:     return "mat4";
    case HLSLBaseType_Float4x3:     return "mat3x4";
    case HLSLBaseType_Float4x2:     return "mat2x4";
    case HLSLBaseType_Float2x3: return "mat3x2";
    case HLSLBaseType_Float2x4: return "mat4x2";
    case HLSLBaseType_Float3x2: return "mat2x3";
    case HLSLBaseType_Float3x4: return "mat4x3";
    case HLSLBaseType_Half:         return "float";
    case HLSLBaseType_Half2:        return "vec2";
    case HLSLBaseType_Half3:        return "vec3";
    case HLSLBaseType_Half4:        return "vec4";
	case HLSLBaseType_Half2x2:      return "mat2";
    case HLSLBaseType_Half3x3:      return "mat3";
    case HLSLBaseType_Half4x4:      return "mat4";
    case HLSLBaseType_Half4x3:      return "mat3x4";
    case HLSLBaseType_Half4x2:      return "mat2x4";
    case HLSLBaseType_Half2x3: return "mat3x2";
    case HLSLBaseType_Half2x4: return "mat4x2";
    case HLSLBaseType_Half3x2: return "mat2x3";
    case HLSLBaseType_Half3x4: return "mat4x3";
    case HLSLBaseType_Bool:         return "bool";
	case HLSLBaseType_Bool2:        return "bvec2";
	case HLSLBaseType_Bool3:        return "bvec3";
	case HLSLBaseType_Bool4:        return "bvec4";
    case HLSLBaseType_Int:          return "int";
    case HLSLBaseType_Int2:         return "ivec2";
    case HLSLBaseType_Int3:         return "ivec3";
    case HLSLBaseType_Int4:         return "ivec4";
    case HLSLBaseType_Uint:         return "uint";
    case HLSLBaseType_Uint2:        return "uvec2";
    case HLSLBaseType_Uint3:        return "uvec3";
    case HLSLBaseType_Uint4:        return "uvec4";
    case HLSLBaseType_Texture:      return "texture";
    case HLSLBaseType_Sampler:      return "sampler";
    case HLSLBaseType_Sampler2D:    return "sampler2D";
    case HLSLBaseType_Sampler3D:    return "sampler3D";
    case HLSLBaseType_SamplerCube:  return "samplerCube";
    case HLSLBaseType_Sampler2DShadow: return "sampler2DShadow";
    case HLSLBaseType_Sampler2DMS:  return "sampler2DMS";
    case HLSLBaseType_Sampler2DArray:  return "sampler2DArray";
    case HLSLBaseType_UserDefined:  return type.typeName;
    default: throw std::invalid_argument("Unsupported HLSL output type");
    }
}

static bool GetCanImplicitCast(const HLSLType& srcType, const HLSLType& dstType)
{
    return srcType.baseType == dstType.baseType && !srcType.array && !dstType.array &&
        (srcType.baseType != HLSLBaseType_UserDefined || String_Equal(srcType.typeName, dstType.typeName));
}

static int GetFunctionArguments(HLSLFunctionCall* functionCall, HLSLExpression* expression[], int maxArguments)
{
    HLSLExpression* argument = functionCall->argument;
    int numArguments = 0;
    while (argument != NULL)
    {
        if (numArguments < maxArguments)
        {
            expression[numArguments] = argument;
        }
        argument = argument->nextExpression;
        ++numArguments;
    }
    return numArguments;
}

GLSLGenerator::GLSLGenerator() :
    m_writer(/* writeFileNames= */ false)
{
    m_tree                      = NULL;
    m_entryName                 = NULL;
    m_target                    = Target_VertexShader;
    m_version                   = Version_140;
    m_versionLegacy             = false;
    m_inAttribPrefix            = NULL;
    m_outAttribPrefix           = NULL;
    m_error                     = false;
    m_matrixRowFunction[0]      = 0;
    m_matrixCtorFunction[0]     = 0;
    m_matrixMulFunction[0]      = 0;
    m_clipFunction[0]           = 0;
    m_tex2DlodFunction[0]       = 0;
    m_tex2DbiasFunction[0]      = 0;
    m_tex3DlodFunction[0]       = 0;
    m_texCUBEbiasFunction[0]    = 0;
	m_texCUBElodFunction[ 0 ] 	= 0;
    m_scalarSwizzle2Function[0] = 0;
    m_scalarSwizzle3Function[0] = 0;
    m_scalarSwizzle4Function[0] = 0;
    m_sinCosFunction[0]         = 0;
	m_bvecTernary[ 0 ]			= 0;
    m_outputPosition            = false;
    m_outputTargets             = 0;
}

bool GLSLGenerator::Generate(HLSLTree* tree, Target target, Version version, const char* entryName, const Options& options)
{

    m_writer.Reset();
    m_numericHelpers.Reset();
    m_numericHelperNames.clear();
    m_result.clear();
    m_error = false;
    m_insideFunction = false;
    m_temporaryCount = 0;
    m_localTemporaries.clear();
    m_milkdropGlobals = false;
    m_globalInitializers.clear();
    m_tree      = tree;
    m_entryName = entryName;
    m_target    = target;
    m_version   = version;
    m_versionLegacy = (version == Version_110 || version == Version_100_ES);
    m_options   = options;

    ChooseUniqueName("matrix_row", m_matrixRowFunction, sizeof(m_matrixRowFunction));
    ChooseUniqueName("matrix_ctor", m_matrixCtorFunction, sizeof(m_matrixCtorFunction));
    ChooseUniqueName("matrix_mul", m_matrixMulFunction, sizeof(m_matrixMulFunction));
    ChooseUniqueName("clip", m_clipFunction, sizeof(m_clipFunction));
    ChooseUniqueName("tex2Dlod", m_tex2DlodFunction, sizeof(m_tex2DlodFunction));
    ChooseUniqueName("tex2Dbias", m_tex2DbiasFunction, sizeof(m_tex2DbiasFunction));
    ChooseUniqueName("tex2Dgrad", m_tex2DgradFunction, sizeof(m_tex2DgradFunction));
    ChooseUniqueName("tex3Dlod", m_tex3DlodFunction, sizeof(m_tex3DlodFunction));
    ChooseUniqueName("texCUBEbias", m_texCUBEbiasFunction, sizeof(m_texCUBEbiasFunction));
	ChooseUniqueName( "texCUBElod", m_texCUBElodFunction, sizeof( m_texCUBElodFunction ) );

    for (int i = 0; i < s_numReservedWords; ++i)
    {
        ChooseUniqueName( s_reservedWord[i], m_reservedWord[i], sizeof(m_reservedWord[i]) );
    }

    ChooseUniqueName("m_scalar_swizzle2", m_scalarSwizzle2Function, sizeof(m_scalarSwizzle2Function));
    ChooseUniqueName("m_scalar_swizzle3", m_scalarSwizzle3Function, sizeof(m_scalarSwizzle3Function));
    ChooseUniqueName("m_scalar_swizzle4", m_scalarSwizzle4Function, sizeof(m_scalarSwizzle4Function));

    ChooseUniqueName("sincos", m_sinCosFunction, sizeof(m_sinCosFunction));

	ChooseUniqueName( "bvecTernary", m_bvecTernary, sizeof( m_bvecTernary ) );

    if (target == Target_VertexShader)
    {
        m_inAttribPrefix  = "";
        m_outAttribPrefix = "frag_";
    }
    else
    {
        m_inAttribPrefix  = "frag_";
        m_outAttribPrefix = "rast_";
    }

    HLSLRoot* root = m_tree->GetRoot();
    HLSLStatement* statement = root->statement;

    // Find the entry point function.
    HLSLFunction* entryFunction = FindFunction(root, m_entryName);
    if (entryFunction == NULL)
    {
        Error("Entry point '%s' doesn't exist", m_entryName);
        return false;
    }

    if (m_version == Version_110)
    {
        m_writer.WriteLine(0, "#version 110");
    }
    else if (m_version == Version_140)
    {
        m_writer.WriteLine(0, "#version 140");

        // Pragmas for NVIDIA.
        m_writer.WriteLine(0, "#pragma optionNV(fastmath on)");
        //m_writer.WriteLine(0, "#pragma optionNV(fastprecision on)");
        m_writer.WriteLine(0, "#pragma optionNV(ifcvt none)");
        m_writer.WriteLine(0, "#pragma optionNV(inline all)");
        m_writer.WriteLine(0, "#pragma optionNV(strict on)");
        m_writer.WriteLine(0, "#pragma optionNV(unroll all)");
    }
    else if (m_version == Version_150)
    {
        m_writer.WriteLine(0, "#version 150");
    }
    else if (m_version == Version_100_ES)
    {
        m_writer.WriteLine(0, "#version 100");
        m_writer.WriteLine(0, "precision highp float;");
        m_writer.WriteLine(0, "precision highp int;");
        m_writer.WriteLine(0, "precision highp sampler3D;");
    }
    else if (m_version == Version_300_ES)
    {
        m_writer.WriteLine(0, "#version 300 es");
        m_writer.WriteLine(0, "precision highp float;");
        m_writer.WriteLine(0, "precision highp int;");
        m_writer.WriteLine(0, "precision highp sampler3D;");
    }
    else
    {
        Error("Unrecognized target version");
        return false;
    }

    if (m_tree->NeedsFunction("fmod")) {
        m_writer.WriteLine(0, "float milkrust_fmod(float x, float y) { return x - y * trunc(x/y); }");
        for (int n = 2; n <= 4; ++n) {
            m_writer.WriteLine(0, "vec%d milkrust_fmod(vec%d x, vec%d y) { return x - y * trunc(x/y); }", n, n, n);
        }
    }

    m_writer.WriteLine(0, "int milkrust_loop_budget;");

    // GLSL ES has no integer or boolean matrix types. Store those HLSL values
    // as row vectors so indexing and component-wise HLSL operators can be
    // lowered without changing integer/boolean precision.
    for (const auto scalar : {HLSLBaseType_Bool, HLSLBaseType_Int, HLSLBaseType_Uint}) {
        const char* scalarName = scalar == HLSLBaseType_Bool ? "bool" : scalar == HLSLBaseType_Int ? "int" : "uint";
        for (int rows = 2; rows <= 4; ++rows) for (int columns = 2; columns <= 4; ++columns) {
            const HLSLType rowType(NumericShape(scalar, 0, columns));
            m_writer.WriteLine(0, "struct milkrust_matrix_%s_%dx%d {", scalarName, rows, columns);
            for (int row = 0; row < rows; ++row)
                m_writer.WriteLine(1, "%s r%d;", GetTypeName(rowType), row);
            m_writer.WriteLine(0, "};");
        }
    }

    // Output the special function used to access rows in a matrix.
    m_writer.WriteLine(0, "vec3 %s(mat3 m, int i) { return vec3( m[0][i], m[1][i], m[2][i] ); }", m_matrixRowFunction);
    m_writer.WriteLine(0, "vec4 %s(mat4 m, int i) { return vec4( m[0][i], m[1][i], m[2][i], m[3][i] ); }", m_matrixRowFunction);

    // Output the special function used to do matrix cast for OpenGL 2.0
    if (m_version == Version_110)
    {
        m_writer.WriteLine(0, "mat3 %s(mat4 m) { return mat3(m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2]); }", m_matrixCtorFunction);
    }

    // Output the special functions used for matrix multiplication lowering
    // They make sure glsl-optimizer can fold expressions better
    if (m_tree->NeedsFunction("mul") && (m_options.flags & Flag_LowerMatrixMultiplication))
    {
        m_writer.WriteLine(0, "vec2 %s(mat2 m, vec2 v) { return m[0] * v.x + m[1] * v.y; }", m_matrixMulFunction);
        m_writer.WriteLine(0, "vec2 %s(vec2 v, mat2 m) { return vec2(dot(m[0], v), dot(m[1], v)); }", m_matrixMulFunction);
        m_writer.WriteLine(0, "vec3 %s(mat3 m, vec3 v) { return m[0] * v.x + m[1] * v.y + m[2] * v.z; }", m_matrixMulFunction);
        m_writer.WriteLine(0, "vec3 %s(vec3 v, mat3 m) { return vec3(dot(m[0], v), dot(m[1], v), dot(m[2], v)); }", m_matrixMulFunction);
        m_writer.WriteLine(0, "vec4 %s(mat4 m, vec4 v) { return m[0] * v.x + m[1] * v.y + m[2] * v.z + m[3] * v.w; }", m_matrixMulFunction);
        m_writer.WriteLine(0, "vec4 %s(vec4 v, mat4 m) { return vec4(dot(m[0], v), dot(m[1], v), dot(m[2], v), dot(m[3], v)); }", m_matrixMulFunction);
    }

    // Output the special function used to emulate HLSL clip.
    if (m_tree->NeedsFunction("clip"))
    {
        const char* discard = m_target == Target_FragmentShader ? "discard" : "";
        m_writer.WriteLine(0, "void %s(float x) { if (x < 0.0) %s;  }", m_clipFunction, discard);
        m_writer.WriteLine(0, "void %s(vec2  x) { if (any(lessThan(x, vec2(0.0, 0.0)))) %s;  }", m_clipFunction, discard);
        m_writer.WriteLine(0, "void %s(vec3  x) { if (any(lessThan(x, vec3(0.0, 0.0, 0.0)))) %s;  }", m_clipFunction, discard);
        m_writer.WriteLine(0, "void %s(vec4  x) { if (any(lessThan(x, vec4(0.0, 0.0, 0.0, 0.0)))) %s;  }", m_clipFunction, discard);
    }

    // Output the special function used to emulate tex2Dlod.
    if (m_tree->NeedsFunction("tex2Dlod"))
    {
        const char* function = "textureLod";

        if (m_version == Version_110)
        {
            m_writer.WriteLine(0, "#extension GL_ARB_shader_texture_lod : require");
            function = "texture2DLod";
        }
        else if (m_version == Version_100_ES)
        {
            m_writer.WriteLine(0, "#extension GL_EXT_shader_texture_lod : require");
            function = "texture2DLodEXT";
        }

        m_writer.WriteLine(0, "vec4 %s(sampler2D samp, vec4 texCoord) { return %s(samp, texCoord.xy, texCoord.w);  }", m_tex2DlodFunction, function);
    }

    // Output the special function used to emulate tex2Dgrad.
    if (m_tree->NeedsFunction("tex2Dgrad"))
    {
        const char* function = "textureGrad";

        if (m_version == Version_110)
        {
            m_writer.WriteLine(0, "#extension GL_ARB_shader_texture_lod : require");
            function = "texture2DGradARB";
        }
        else if (m_version == Version_100_ES)
        {
            m_writer.WriteLine(0, "#extension GL_EXT_shader_texture_lod : require");
            function = "texture2DGradEXT";
        }

        m_writer.WriteLine(0, "vec4 %s(sampler2D samp, vec2 texCoord, vec2 dx, vec2 dy) { return %s(samp, texCoord, dx, dy);  }", m_tex2DgradFunction, function);
    }

    // Output the special function used to emulate tex2Dbias.
    if (m_tree->NeedsFunction("tex2Dbias"))
    {
        if (target == Target_FragmentShader)
        {
            m_writer.WriteLine(0, "vec4 %s(sampler2D samp, vec4 texCoord) { return %s(samp, texCoord.xy, texCoord.w);  }", m_tex2DbiasFunction, m_versionLegacy ? "texture2D" : "texture" );
        }
        else
        {
            // Bias value is not supported in vertex shader.
            m_writer.WriteLine(0, "vec4 %s(sampler2D samp, vec4 texCoord) { return texture(samp, texCoord.xy);  }", m_tex2DbiasFunction );
        }
    }

    // Output the special function used to emulate tex2DMSfetch.
    if (m_tree->NeedsFunction("tex2DMSfetch"))
    {
        m_writer.WriteLine(0, "vec4 tex2DMSfetch(sampler2DMS samp, ivec2 texCoord, int sample) {");
        m_writer.WriteLine(1, "return texelFetch(samp, texCoord, sample);");
        m_writer.WriteLine(0, "}");
    }

    // Output the special function used to emulate tex3Dlod.
    if (m_tree->NeedsFunction("tex3Dlod"))
    {
        m_writer.WriteLine(0, "vec4 %s(sampler3D samp, vec4 texCoord) { return %s(samp, texCoord.xyz, texCoord.w);  }", m_tex3DlodFunction, m_versionLegacy ? "texture3D" : "texture" );
    }

    // Output the special function used to emulate texCUBEbias.
    if (m_tree->NeedsFunction("texCUBEbias"))
    {
        if (target == Target_FragmentShader)
        {
            m_writer.WriteLine(0, "vec4 %s(samplerCube samp, vec4 texCoord) { return %s(samp, texCoord.xyz, texCoord.w);  }", m_texCUBEbiasFunction, m_versionLegacy ? "textureCube" : "texture" );
        }
        else
        {
            // Bias value is not supported in vertex shader.
            m_writer.WriteLine(0, "vec4 %s(samplerCube samp, vec4 texCoord) { return texture(samp, texCoord.xyz);  }", m_texCUBEbiasFunction );
        }
    }

	// Output the special function used to emulate texCUBElod
	if (m_tree->NeedsFunction("texCUBElod"))
	{
        const char* function = "textureLod";

        if (m_version == Version_110)
        {
            m_writer.WriteLine(0, "#extension GL_ARB_shader_texture_lod : require");
            function = "textureCubeLod";
        }
        else if (m_version == Version_100_ES)
        {
            m_writer.WriteLine(0, "#extension GL_EXT_shader_texture_lod : require");
            function = "textureCubeLodEXT";
        }

		m_writer.WriteLine( 0, "vec4 %s(samplerCube samp, vec4 texCoord) { return %s(samp, texCoord.xyz, texCoord.w);  }", m_texCUBElodFunction, function);
	}

    m_writer.WriteLine(0, "vec2  %s(float x) { return  vec2(x, x); }", m_scalarSwizzle2Function);
    m_writer.WriteLine(0, "ivec2 %s(int   x) { return ivec2(x, x); }", m_scalarSwizzle2Function);

    m_writer.WriteLine(0, "vec3  %s(float x) { return  vec3(x, x, x); }", m_scalarSwizzle3Function);
    m_writer.WriteLine(0, "ivec3 %s(int   x) { return ivec3(x, x, x); }", m_scalarSwizzle3Function);

    m_writer.WriteLine(0, "vec4  %s(float x) { return  vec4(x, x, x, x); }", m_scalarSwizzle4Function);
    m_writer.WriteLine(0, "ivec4 %s(int   x) { return ivec4(x, x, x, x); }", m_scalarSwizzle4Function);

    m_writer.WriteLine(0, "bvec2 %s(bool x) { return bvec2(x); }", m_scalarSwizzle2Function);
    m_writer.WriteLine(0, "bvec3 %s(bool x) { return bvec3(x); }", m_scalarSwizzle3Function);
    m_writer.WriteLine(0, "bvec4 %s(bool x) { return bvec4(x); }", m_scalarSwizzle4Function);

    if (!m_versionLegacy)
    {
        m_writer.WriteLine(0, "uvec2 %s(uint  x) { return uvec2(x, x); }", m_scalarSwizzle2Function);
        m_writer.WriteLine(0, "uvec3 %s(uint  x) { return uvec3(x, x, x); }", m_scalarSwizzle3Function);
        m_writer.WriteLine(0, "uvec4 %s(uint  x) { return uvec4(x, x, x, x); }", m_scalarSwizzle4Function);
    }

    if (m_tree->NeedsFunction("sincos"))
    {
        const char* floatTypes[] = { "float", "vec2", "vec3", "vec4" };
        for (int i = 0; i < 4; ++i)
        {
            m_writer.WriteLine(0, "void %s(%s x, out %s s, out %s c) { s = sin(x); c = cos(x); }", m_sinCosFunction,
                floatTypes[i], floatTypes[i], floatTypes[i]);
        }
    }

    // Legacy vector conditionals evaluate and select each component.
    const char* vectorKinds[] = {"vec", "ivec", "uvec", "bvec"};
    for (const char* kind : vectorKinds) for (int width = 2; width <= 4; ++width) {
        if (m_versionLegacy && strcmp(kind, "uvec") == 0) continue;
        m_writer.Write("%s%d %s(bvec%d c, %s%d a, %s%d b) { return %s%d(", kind, width, m_bvecTernary,
            width, kind, width, kind, width, kind, width);
        for (int i = 0; i < width; ++i) { if (i) m_writer.Write(","); m_writer.Write("c[%d] ? a[%d] : b[%d]", i, i, i); }
        m_writer.EndLine("); }");
    }

    // Output the extension used for dFdx/dFdy in GLES2
    if (m_version == Version_100_ES && (m_tree->NeedsFunction("ddx") || m_tree->NeedsFunction("ddy")))
    {
        m_writer.WriteLine(0, "#extension GL_OES_standard_derivatives : require");
    }

    OutputAttributes(entryFunction);

    if (m_target == Target_FragmentShader)
    {
        if (!m_outputTargets)
            Error("Fragment shader must output a color");

        if (!m_versionLegacy)
            m_writer.WriteLine(0, "out vec4 rast_FragData[%d];", m_outputTargets);
    }

    // Structs (including array row wrappers) must precede helpers and globals
    // that use them. Keep type dependencies in order instead of placing helpers
    // before their parameter/return struct definitions.
    class Types : public HLSLTreeVisitor {
    public:
        std::vector<HLSLType> values;
        void VisitType(HLSLType& type) override {
            CheckBudget();
            if (type.arrayRank > 1 || type.baseType == HLSLBaseType_UserDefined) values.push_back(type);
        }
        void VisitStruct(HLSLStruct* node) override {
            HLSLType type(HLSLBaseType_UserDefined); type.typeName = node->name;
            VisitType(type); HLSLTreeVisitor::VisitStruct(node);
        }
        void VisitConstructorExpression(HLSLConstructorExpression* node) override {
            VisitType(node->type); HLSLTreeVisitor::VisitConstructorExpression(node);
        }
    } types;
    types.VisitRoot(root);
    m_typeDefinitions.clear(); m_pendingTypeDefinitions.clear();
    for (const auto& type : types.values) OutputTypeDefinition(type);
    const size_t helperOffset = strlen(m_writer.GetResult());
    OutputStatements(0, statement);
    OutputEntryCaller(entryFunction);
    m_result = m_writer.GetResult();
    m_result.insert(helperOffset, m_numericHelpers.GetResult());
    if (m_result.size() > 1024 * 1024) throw std::length_error("GLSL output limit exceeded");

    m_tree = NULL;

    // The GLSL compilers don't check for this, so generate our own error message.
    if (target == Target_VertexShader && !m_outputPosition)
    {
        Error("Vertex shader must output a position");
    }

    return !m_error;

}

const char* GLSLGenerator::GetResult() const
{
    return m_result.c_str();
}

void GLSLGenerator::OutputExpressionList(HLSLExpression* expression, HLSLArgument* argument)
{
    DepthGuard milkrustDepth;
    int numExpressions = 0;
    while (expression != NULL)
    {
        if (numExpressions > 0)
        {
            m_writer.Write(", ");
        }

        HLSLType* expectedType = NULL;
        if (argument != NULL)
        {
            expectedType = &argument->type;
            argument = argument->nextArgument;
        }

        OutputExpression(expression, expectedType);
        expression = expression->nextExpression;
        ++numExpressions;
    }
}

#include "GLSLNumeric.inl"
#include "GLSLIntrinsics.inl"
#include "GLSLAggregates.inl"

void GLSLGenerator::OutputExpression(HLSLExpression* expression, const HLSLType* dstType)
{
    DepthGuard milkrustDepth;

    if (dstType && !GetCanImplicitCast(expression->expressionType, *dstType)) {
        OutputConversion(expression, *dstType);
        return;
    }
    if (OutputMatrixSelection(expression)) return;

    HLSLBuffer* bufferAccess = (m_options.flags & Flag_EmulateConstantBuffer) ? GetBufferAccessExpression(expression) : 0;

    if (bufferAccess)
    {
        OutputBufferAccessExpression(bufferAccess, expression, expression->expressionType, 0);
    }
    else if (expression->nodeType == HLSLNodeType_IdentifierExpression)
    {
        HLSLIdentifierExpression* identifierExpression = static_cast<HLSLIdentifierExpression*>(expression);
        // Value identifiers must not pass through the intrinsic-name mapper:
        // a local named frac or tex2D is a variable, not a sampling call.
        m_writer.Write("%s", GetSafeIdentifierName(identifierExpression->name));
    }
    else if (expression->nodeType == HLSLNodeType_ConstructorExpression)
    {
        HLSLConstructorExpression* constructorExpression = static_cast<HLSLConstructorExpression*>(expression);
        if (constructorExpression->initializer) {
            OutputAggregateConstructor(constructorExpression); return;
        }
        if (IsMatrixType(constructorExpression->type.baseType)) {
            OutputMatrixConstructor(constructorExpression); return;
        }
        m_writer.Write("%s(", GetTypeName(constructorExpression->type));
        if (constructorExpression->type.baseType == HLSLBaseType_UserDefined) {
            auto* structure = FindStruct(m_tree->GetRoot(), constructorExpression->type.typeName);
            auto* field = structure ? structure->field : NULL;
            auto* argument = constructorExpression->argument;
            for (; field && argument; field = field->nextField, argument = argument->nextExpression) {
                OutputExpression(argument, &field->type);
                if (argument->nextExpression) m_writer.Write(", ");
            }
            if (field || argument || !structure) Error("Structure constructor arity mismatch");
        } else OutputExpressionList(constructorExpression->argument);
        m_writer.Write(")");
    }
    else if (expression->nodeType == HLSLNodeType_CastingExpression)
    {
        HLSLCastingExpression* castingExpression = static_cast<HLSLCastingExpression*>(expression);
        OutputConversion(castingExpression->expression, castingExpression->type);
    }
    else if (expression->nodeType == HLSLNodeType_LiteralExpression)
    {
        HLSLLiteralExpression* literalExpression = static_cast<HLSLLiteralExpression*>(expression);
        switch (literalExpression->type)
        {
        case HLSLBaseType_Half:
        case HLSLBaseType_Float:
            {
                // Don't use printf directly so that we don't use the system locale.
                char buffer[64];
                String_FormatFloat(buffer, sizeof(buffer), literalExpression->fValue);
                m_writer.Write("%s", buffer);
            }
            break;
        case HLSLBaseType_Int:
            m_writer.Write("%d", literalExpression->iValue);
            break;
        case HLSLBaseType_Uint:
            m_writer.Write("%uu", static_cast<unsigned int>(literalExpression->iValue));
	    break;
	case HLSLBaseType_Bool:
            m_writer.Write("%s", literalExpression->bValue ? "true" : "false");
            break;
        default:
            ASSERT(0);
        }
    }
    else if (expression->nodeType == HLSLNodeType_UnaryExpression)
    {
        HLSLUnaryExpression* unaryExpression = static_cast<HLSLUnaryExpression*>(expression);
        const char* op = "?";
        bool pre = true;
        const HLSLType* dstType = NULL;
        switch (unaryExpression->unaryOp)
        {
        case HLSLUnaryOp_Negative:      op = "-";  break;
        case HLSLUnaryOp_Positive:      op = "+";  break;
        case HLSLUnaryOp_Not:           op = "!";  dstType = &unaryExpression->expressionType; break;
        case HLSLUnaryOp_PreIncrement:  op = "++"; break;
        case HLSLUnaryOp_PreDecrement:  op = "--"; break;
        case HLSLUnaryOp_PostIncrement: op = "++"; pre = false; break;
        case HLSLUnaryOp_PostDecrement: op = "--"; pre = false; break;
        case HLSLUnaryOp_BitNot:        op = "~";  break;
        }
        if ((unaryExpression->unaryOp == HLSLUnaryOp_PreIncrement || unaryExpression->unaryOp == HLSLUnaryOp_PostIncrement ||
             unaryExpression->unaryOp == HLSLUnaryOp_PreDecrement || unaryExpression->unaryOp == HLSLUnaryOp_PostDecrement) &&
            OutputMatrixSelection(unaryExpression->expression, NULL, op[0] == '+' ? "+" : "-", !pre)) return;
        const HLSLType& sourceType = unaryExpression->expression->expressionType;
        const HLSLType& resultType = unaryExpression->expressionType;
        if (IsMatrixType(sourceType.baseType) &&
            (IsCustomMatrixType(sourceType.baseType) || IsCustomMatrixType(resultType.baseType) || unaryExpression->unaryOp == HLSLUnaryOp_Not)) {
            const int rows = MatrixRows(resultType.baseType), columns = NumericWidth(resultType.baseType);
            const int sourceRows = MatrixRows(sourceType.baseType), sourceColumns = NumericWidth(sourceType.baseType);
            const bool increment = unaryExpression->unaryOp == HLSLUnaryOp_PreIncrement || unaryExpression->unaryOp == HLSLUnaryOp_PostIncrement ||
                unaryExpression->unaryOp == HLSLUnaryOp_PreDecrement || unaryExpression->unaryOp == HLSLUnaryOp_PostDecrement;
            const std::string inputType = GetTypeName(sourceType), outputType = GetTypeName(resultType);
            const std::string parameters = std::string(increment ? "inout " : "") + inputType + " a";
            std::string body;
            auto valueAt = [&](int row, int column) {
                const std::string value = MatrixComponent(sourceType, "a", row, column);
                if (unaryExpression->unaryOp == HLSLUnaryOp_Not) return "(!bool(" + value + "))";
                if (unaryExpression->unaryOp == HLSLUnaryOp_Negative) return "(-" + value + ")";
                if (unaryExpression->unaryOp == HLSLUnaryOp_BitNot) return "(~" + value + ")";
                return value;
            };
            if (increment) body = inputType + " oldValue = a; ";
            if (IsCustomMatrixType(resultType.baseType)) {
                if (!increment) body += outputType + " result; ";
                const HLSLType rowType(NumericShape(ScalarBaseType[resultType.baseType], 0, columns));
                for (int r = 0; r < rows; ++r) {
                    if (increment) body += "a.r" + std::to_string(r) + (op[0] == '+' ? "+=" : "-=") + GetTypeName(rowType) + "(1); ";
                    else {
                        body += "result.r" + std::to_string(r) + "=" + GetTypeName(rowType) + "(";
                        for (int c = 0; c < columns; ++c) { if (c) body += ","; body += valueAt(r, c); }
                        body += "); ";
                    }
                }
                body += increment ? (std::string("return ") + (pre ? "a;" : "oldValue;")) : "return result;";
            } else {
                const int count = rows == 1 ? columns : columns == 1 ? rows : rows * columns;
                body += "return " + outputType + "(";
                for (int i = 0; i < count; ++i) {
                    if (i) body += ",";
                    const int r = rows == 1 ? 0 : columns == 1 ? i : i / columns;
                    const int c = rows == 1 ? i : columns == 1 ? 0 : i % columns;
                    body += valueAt(r, c);
                }
                body += ");";
            }
            const std::string helper = NumericHelper("matrix-unary:" + inputType + outputType + parameters + body,
                outputType, parameters, body);
            m_writer.Write("%s(", helper.c_str()); OutputExpression(unaryExpression->expression); m_writer.Write(")"); return;
        }
        if (unaryExpression->unaryOp == HLSLUnaryOp_Not && IsVectorType(unaryExpression->expressionType)) {
            m_writer.Write("not("); OutputExpression(unaryExpression->expression, &unaryExpression->expressionType); m_writer.Write(")"); return;
        }
        m_writer.Write("(");
        if (pre)
        {
            m_writer.Write("%s", op);
            OutputExpression(unaryExpression->expression, dstType);
        }
        else
        {
            OutputExpression(unaryExpression->expression, dstType);
            m_writer.Write("%s", op);
        }
        m_writer.Write(")");
    }
    else if (expression->nodeType == HLSLNodeType_BinaryExpression)
    {
        HLSLBinaryExpression* binaryExpression = static_cast<HLSLBinaryExpression*>(expression);
        const char* op = "?";
        const HLSLType* dstType1 = NULL;
        const HLSLType* dstType2 = NULL;

        if (IsAssignOp(binaryExpression->binaryOp)) {
            const char* operation = binaryExpression->binaryOp == HLSLBinaryOp_Assign ? "=" :
                binaryExpression->binaryOp == HLSLBinaryOp_AddAssign ? "+" : binaryExpression->binaryOp == HLSLBinaryOp_SubAssign ? "-" :
                binaryExpression->binaryOp == HLSLBinaryOp_MulAssign ? "*" : binaryExpression->binaryOp == HLSLBinaryOp_DivAssign ? "/" :
                binaryExpression->binaryOp == HLSLBinaryOp_ModAssign ? "%" : binaryExpression->binaryOp == HLSLBinaryOp_BitAndAssign ? "&" :
                binaryExpression->binaryOp == HLSLBinaryOp_BitOrAssign ? "|" : binaryExpression->binaryOp == HLSLBinaryOp_BitXorAssign ? "^" :
                binaryExpression->binaryOp == HLSLBinaryOp_LeftShiftAssign ? "<<" : ">>";
            if (OutputMatrixSelection(binaryExpression->expression1, binaryExpression->expression2, operation)) return;
        }
        const auto lhsBase = binaryExpression->expression1->expressionType.baseType;
        const auto rhsBase = binaryExpression->expression2->expressionType.baseType;
        const bool hasMatrix = IsMatrixType(lhsBase) || IsMatrixType(rhsBase) || IsMatrixType(binaryExpression->expressionType.baseType);
        if (hasMatrix && binaryExpression->binaryOp != HLSLBinaryOp_Assign &&
                (IsCustomMatrixType(lhsBase) || IsCustomMatrixType(rhsBase) ||
                IsCustomMatrixType(binaryExpression->expressionType.baseType) ||
                IsCompareOp(binaryExpression->binaryOp) || IsLogicOp(binaryExpression->binaryOp))) {
            const bool assign = IsAssignOp(binaryExpression->binaryOp) && binaryExpression->binaryOp != HLSLBinaryOp_Assign;
            const bool boolComparison = (binaryExpression->binaryOp == HLSLBinaryOp_Equal || binaryExpression->binaryOp == HLSLBinaryOp_NotEqual) &&
                ScalarBaseType[lhsBase] == HLSLBaseType_Bool && ScalarBaseType[rhsBase] == HLSLBaseType_Bool;
            const HLSLBaseType comparisonScalar = boolComparison
                ? lhsBase : CommonNumericType(lhsBase, rhsBase);
            HLSLType operandType(assign || IsLogicOp(binaryExpression->binaryOp) ? lhsBase : comparisonScalar);
            if (operandType.baseType == HLSLBaseType_Unknown) throw std::invalid_argument("Invalid HLSL matrix operator type");
            const HLSLType& resultType = binaryExpression->expressionType;
            const int rows = MatrixRows(resultType.baseType), columns = NumericWidth(resultType.baseType);
            const int vectorWidth = rows == 1 ? columns : columns == 1 ? rows : 0;
            if (!rows || !columns) throw std::invalid_argument("Invalid HLSL matrix operator shape");
            const std::string resultName = GetTypeName(resultType);
            const std::string operandName = GetTypeName(operandType);
            std::string op;
            switch (binaryExpression->binaryOp) {
                case HLSLBinaryOp_Add: case HLSLBinaryOp_AddAssign: op = "+"; break;
                case HLSLBinaryOp_Sub: case HLSLBinaryOp_SubAssign: op = "-"; break;
                case HLSLBinaryOp_Mul: case HLSLBinaryOp_MulAssign: op = "*"; break;
                case HLSLBinaryOp_Div: case HLSLBinaryOp_DivAssign: op = "/"; break;
                case HLSLBinaryOp_Mod: case HLSLBinaryOp_ModAssign: op = "%"; break;
                case HLSLBinaryOp_BitAnd: case HLSLBinaryOp_BitAndAssign: op = "&"; break;
                case HLSLBinaryOp_BitOr: case HLSLBinaryOp_BitOrAssign: op = "|"; break;
                case HLSLBinaryOp_BitXor: case HLSLBinaryOp_BitXorAssign: op = "^"; break;
                case HLSLBinaryOp_LeftShift: case HLSLBinaryOp_LeftShiftAssign: op = "<<"; break;
                case HLSLBinaryOp_RightShift: case HLSLBinaryOp_RightShiftAssign: op = ">>"; break;
                case HLSLBinaryOp_And: op = "&&"; break;
                case HLSLBinaryOp_Or: op = "||"; break;
                case HLSLBinaryOp_Less: op = "<"; break;
                case HLSLBinaryOp_Greater: op = ">"; break;
                case HLSLBinaryOp_LessEqual: op = "<="; break;
                case HLSLBinaryOp_GreaterEqual: op = ">="; break;
                case HLSLBinaryOp_Equal: op = "=="; break;
                case HLSLBinaryOp_NotEqual: op = "!="; break;
                default: throw std::invalid_argument("Unsupported HLSL matrix operator");
            }
            const std::string leftType = assign ? GetTypeName(binaryExpression->expression1->expressionType) : operandName;
            const std::string rightType = assign ? leftType : operandName;
            const std::string parameters = std::string(assign ? "inout " : "") + leftType + " a," + rightType + " b";
            const bool comparison = IsCompareOp(binaryExpression->binaryOp);
            const bool logical = IsLogicOp(binaryExpression->binaryOp);
            auto component = [](const HLSLType& type, const std::string& value, int row, int column) {
                if (IsMatrixType(type.baseType)) {
                    const int r = MatrixRows(type.baseType), c = NumericWidth(type.baseType);
                    if (IsCustomMatrixType(type.baseType))
                        return value + ".r" + std::to_string(row) + "[" + std::to_string(column) + "]";
                    if (r > 1 && c > 1)
                        return value + "[" + std::to_string(column) + "][" + std::to_string(row) + "]";
                    const int index = r == 1 ? column : row;
                    return (r == 1 && c == 1) || (r > 1 && c == 1) && index == 0
                        ? value : value + "[" + std::to_string(index) + "]";
                }
                return IsScalarType(type) ? value : value + "[" + std::to_string(column) + "]";
            };
            const HLSLType& leftExpressionType = binaryExpression->expression1->expressionType;
            const HLSLType& rightExpressionType = binaryExpression->expression2->expressionType;
            std::string body;
            if (vectorWidth) {
                const int count = vectorWidth;
                const HLSLBaseType scalar = ScalarBaseType[resultType.baseType];
                const std::string vectorType = GetTypeName(HLSLType(NumericShape(scalar, 0, count)));
                body = "return " + vectorType + "(";
                for (int c = 0; c < count; ++c) {
                    if (c) body += ",";
                    const std::string a = component(assign ? binaryExpression->expression1->expressionType : operandType, "a", rows == 1 ? 0 : c, rows == 1 ? c : 0);
                    const std::string b = component(assign ? binaryExpression->expression1->expressionType : operandType, "b", rows == 1 ? 0 : c, rows == 1 ? c : 0);
                    body += logical ? "(bool(" + a + ")" + op + "bool(" + b + "))" : "(" + a + op + b + ")";
                }
                body += ");";
            } else if (IsCustomMatrixType(resultType.baseType)) {
                const HLSLType rowType(NumericShape(ScalarBaseType[resultType.baseType], 0, columns));
                if (!assign) body = resultName + " result;";
                for (int r = 0; r < rows; ++r) {
                    if (comparison || logical) {
                        body += "result.r" + std::to_string(r) + "=bvec" + std::to_string(columns) + "(";
                        for (int c = 0; c < columns; ++c) {
                            if (c) body += ",";
                            const std::string a = component(operandType, "a", r, c), b = component(operandType, "b", r, c);
                            body += logical ? "(bool(" + a + ")" + op + "bool(" + b + "))" : "(" + a + op + b + ")";
                        }
                        body += ");";
                    } else if (assign) {
                        body += "a.r" + std::to_string(r) + "=a.r" + std::to_string(r) + op + "b.r" + std::to_string(r) + ";";
                    } else {
                        const std::string rowName = GetTypeName(rowType);
                        body += "result.r" + std::to_string(r) + "=" + rowName + "(";
                        for (int c = 0; c < columns; ++c) {
                            if (c) body += ",";
                            body += "(" + component(operandType, "a", r, c) + op + component(operandType, "b", r, c) + ")";
                        }
                        body += ");";
                    }
                }
                body += assign ? "return a;" : "return result;";
            } else if (IsMatrixType(resultType.baseType) && rows > 1 && columns > 1 && !comparison && !logical) {
                body = "return " + resultName + "(";
                for (int c = 0; c < columns; ++c) for (int r = 0; r < rows; ++r) {
                    if (c || r) body += ",";
                    body += "(" + component(operandType, "a", r, c) + op + component(operandType, "b", r, c) + ")";
                }
                body += ");";
            } else {
                throw std::invalid_argument("Unsupported HLSL matrix operator shape");
            }
            const std::string helper = NumericHelper("matrix-op:" + resultName + parameters + op + body,
                resultName, parameters, body);
            m_writer.Write("%s(", helper.c_str());
            OutputExpression(binaryExpression->expression1, &operandType);
            m_writer.Write(", "); OutputExpression(binaryExpression->expression2, &operandType);
            m_writer.Write(")"); return;
        }
        if (IsMatrixType(binaryExpression->expressionType.baseType) &&
            MatrixRows(binaryExpression->expressionType.baseType) > 1 && NumericWidth(binaryExpression->expressionType.baseType) > 1 &&
            (binaryExpression->binaryOp == HLSLBinaryOp_Mul || binaryExpression->binaryOp == HLSLBinaryOp_MulAssign)) {
            const bool assign = binaryExpression->binaryOp == HLSLBinaryOp_MulAssign;
            const std::string type = GetTypeName(binaryExpression->expressionType);
            const std::string name = assign ? NumericHelper("multiplyassign:" + type, type,
                "inout " + type + " a, " + type + " b", "a = matrixCompMult(a,b); return a;") : "matrixCompMult";
            m_writer.Write("%s(", name.c_str());
            OutputExpression(binaryExpression->expression1, assign ? NULL : &binaryExpression->expressionType);
            m_writer.Write(", "); OutputExpression(binaryExpression->expression2, &binaryExpression->expressionType); m_writer.Write(")"); return;
        }
        if ((binaryExpression->binaryOp == HLSLBinaryOp_Mod || binaryExpression->binaryOp == HLSLBinaryOp_ModAssign) &&
            (!IsMatrixType(binaryExpression->expressionType.baseType) || MatrixRows(binaryExpression->expressionType.baseType) == 1 || NumericWidth(binaryExpression->expressionType.baseType) == 1) &&
            (ScalarBaseType[binaryExpression->expressionType.baseType] == HLSLBaseType_Float || ScalarBaseType[binaryExpression->expressionType.baseType] == HLSLBaseType_Half)) {
            const bool assign = binaryExpression->binaryOp == HLSLBinaryOp_ModAssign;
            const std::string type = GetTypeName(binaryExpression->expressionType);
            std::string value = "a-b*trunc(a/b)";
            if (IsMatrixType(binaryExpression->expressionType.baseType) && MatrixRows(binaryExpression->expressionType.baseType) > 1 && NumericWidth(binaryExpression->expressionType.baseType) > 1) {
                value = type + "(";
                for (int c = 0; c < NumericWidth(binaryExpression->expressionType.baseType); ++c) {
                    if (c) value += ",";
                    const std::string column = "[" + std::to_string(c) + "]";
                    value += "a" + column + "-b" + column + "*trunc(a" + column + "/b" + column + ")";
                }
                value += ")";
            }
            const auto name = NumericHelper("remainder:" + type + (assign ? "assign" : ""), type,
                (assign ? "inout " : "") + type + " a," + type + " b", assign ? "a=" + value + ";return a;" : "return " + value + ";");
            m_writer.Write("%s(", name.c_str()); OutputExpression(binaryExpression->expression1, assign ? NULL : &binaryExpression->expressionType);
            m_writer.Write(", "); OutputExpression(binaryExpression->expression2, &binaryExpression->expressionType); m_writer.Write(")"); return;
        }
        HLSLType commonType(CommonNumericType(binaryExpression->expression1->expressionType.baseType, binaryExpression->expression2->expressionType.baseType));
        if (IsLogicOp(binaryExpression->binaryOp) && IsVectorType(binaryExpression->expressionType)) {
            const int width = NumericWidth(binaryExpression->expressionType.baseType);
            const std::string type = GetTypeName(binaryExpression->expressionType);
            std::string body = "return " + type + "(";
            for (int i = 0; i < width; ++i) {
                if (i) body += ",";
                body += "a[" + std::to_string(i) + "]" + (binaryExpression->binaryOp == HLSLBinaryOp_And ? " && " : " || ") + "b[" + std::to_string(i) + "]";
            }
            body += ");";
            const auto name = NumericHelper(type + body, type, type + " a," + type + " b", body);
            m_writer.Write("%s(", name.c_str()); OutputExpression(binaryExpression->expression1, &binaryExpression->expressionType);
            m_writer.Write(", "); OutputExpression(binaryExpression->expression2, &binaryExpression->expressionType); m_writer.Write(")"); return;
        }

		//
		bool vectorExpression = IsVectorType( binaryExpression->expression1->expressionType ) || IsVectorType( binaryExpression->expression2->expressionType );
		if( vectorExpression && IsCompareOp( binaryExpression->binaryOp ))
		{
			switch (binaryExpression->binaryOp)
			{
			case HLSLBinaryOp_Less:         m_writer.Write("lessThan(");			break;
			case HLSLBinaryOp_Greater:      m_writer.Write("greaterThan(");			break;
			case HLSLBinaryOp_LessEqual:    m_writer.Write("lessThanEqual(");		break;
			case HLSLBinaryOp_GreaterEqual: m_writer.Write("greaterThanEqual(");	break;
			case HLSLBinaryOp_Equal:        m_writer.Write("equal(");				break;
			case HLSLBinaryOp_NotEqual:     m_writer.Write("notEqual(");			break;
			default:
				ASSERT(0); // is so, check isCompareOp
			}

            dstType1 = dstType2 = &commonType;

			OutputExpression(binaryExpression->expression1, dstType1);
			m_writer.Write(", ");
			OutputExpression(binaryExpression->expression2, dstType2);
			m_writer.Write(")");
		}
		else
		{
			switch (binaryExpression->binaryOp)
			{
			case HLSLBinaryOp_Add:          op = " + "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Sub:          op = " - "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Mul:          op = " * "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Div:          op = " / "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Less:         op = " < "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_Greater:      op = " > "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_LessEqual:    op = " <= "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_GreaterEqual: op = " >= "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_Equal:        op = " == "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_NotEqual:     op = " != "; dstType1 = dstType2 = &commonType; break;
            case HLSLBinaryOp_Mod: op = " % "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_LeftShift: op = " << "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_RightShift: op = " >> "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_ModAssign: op = " %= "; dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_BitAndAssign: op = " &= "; dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_BitOrAssign: op = " |= "; dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_BitXorAssign: op = " ^= "; dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_LeftShiftAssign: op = " <<= "; dstType2 = &binaryExpression->expressionType; break;
            case HLSLBinaryOp_RightShiftAssign: op = " >>= "; dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Assign:       op = " = ";  dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_AddAssign:    op = " += "; dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_SubAssign:    op = " -= "; dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_MulAssign:    op = " *= "; dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_DivAssign:    op = " /= "; dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_And:          op = " && "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_Or:           op = " || "; dstType1 = dstType2 = &binaryExpression->expressionType; break;
			case HLSLBinaryOp_BitAnd:       op = " & "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_BitOr:        op = " | "; dstType1 = dstType2 = &commonType; break;
			case HLSLBinaryOp_BitXor:       op = " ^ "; dstType1 = dstType2 = &commonType; break;
			default:
				ASSERT(0);
			}
			m_writer.Write("(");
			OutputExpression(binaryExpression->expression1, dstType1);
			m_writer.Write("%s", op);
			OutputExpression(binaryExpression->expression2, dstType2);
			m_writer.Write(")");
		}
    }
    else if (expression->nodeType == HLSLNodeType_ConditionalExpression)
    {
        HLSLConditionalExpression* conditionalExpression = static_cast<HLSLConditionalExpression*>(expression);
		if( IsVectorType( conditionalExpression->condition->expressionType ) )
		{
			m_writer.Write( "%s", m_bvecTernary );
			m_writer.Write( "( " );
            HLSLType conditionType(NumericShape(HLSLBaseType_Bool, 0, NumericWidth(conditionalExpression->expressionType.baseType)));
            OutputExpression(conditionalExpression->condition, &conditionType);
			m_writer.Write( ", " );
			OutputExpression( conditionalExpression->trueExpression, &conditionalExpression->expressionType );
			m_writer.Write( ", " );
			OutputExpression( conditionalExpression->falseExpression, &conditionalExpression->expressionType  );
			m_writer.Write( " )" );
		}
		else
		{
			m_writer.Write( "((" );
			OutputExpression( conditionalExpression->condition, &kBoolType );
			m_writer.Write( ")?(" );
			OutputExpression( conditionalExpression->trueExpression, &conditionalExpression->expressionType );
			m_writer.Write( "):(" );
			OutputExpression( conditionalExpression->falseExpression, &conditionalExpression->expressionType );
			m_writer.Write( "))" );
		}
    }
    else if (expression->nodeType == HLSLNodeType_MemberAccess)
    {

        HLSLMemberAccess* memberAccess = static_cast<HLSLMemberAccess*>(expression);

        if (IsScalarType(memberAccess->object->expressionType)) {
            if (strlen(memberAccess->field) > 1) m_writer.Write("%s", GetTypeName(memberAccess->expressionType));
            m_writer.Write("("); OutputExpression(memberAccess->object); m_writer.Write(")");
        }
        else
        {
            m_writer.Write("(");
            OutputExpression(memberAccess->object);
            m_writer.Write(")");

            m_writer.Write(".%s", memberAccess->field);

        }

    }
    else if (expression->nodeType == HLSLNodeType_ArrayAccess)
    {
        HLSLArrayAccess* arrayAccess = static_cast<HLSLArrayAccess*>(expression);

        OutputExpression(arrayAccess->array);
        m_writer.Write("["); OutputExpression(arrayAccess->index, &kIntType); m_writer.Write("]");
        if (arrayAccess->array->expressionType.arrayRank > 1) m_writer.Write(".elements");

    }
    else if (expression->nodeType == HLSLNodeType_FunctionCall)
    {
        HLSLFunctionCall* functionCall = static_cast<HLSLFunctionCall*>(expression);

        // Handle intrinsic funtions that are different between HLSL and GLSL.
        bool handled = false;
        if (OutputCallWithCopyOut(functionCall)) return;
        if (!functionCall->function->intrinsic) {
            m_writer.Write("%s(", GetSafeIdentifierName(functionCall->function->name));
            OutputExpressionList(functionCall->argument, functionCall->function->argument); m_writer.Write(")"); return;
        }
        if (OutputLegacyIntrinsic(functionCall)) return;
        const char* functionName = functionCall->function->name;

        if (String_Equal(functionName, "mul"))
        {
            HLSLExpression* argument[2];
            if (GetFunctionArguments(functionCall, argument, 2) != 2)
            {
                Error("mul expects 2 arguments");
                return;
            }

            const HLSLType& type0 = functionCall->function->argument->type;
            const HLSLType& type1 = functionCall->function->argument->nextArgument->type;

            if (IsVectorType(type0) && IsVectorType(type1)) {
                m_writer.Write("dot("); OutputExpression(argument[0], &type0);
                m_writer.Write(", "); OutputExpression(argument[1], &type1); m_writer.Write(")"); return;
            }
            const char* prefix = (m_options.flags & Flag_LowerMatrixMultiplication) ? m_matrixMulFunction : "";
            const char* infix = (m_options.flags & Flag_LowerMatrixMultiplication) ? "," : "*";

            if (m_options.flags & Flag_PackMatrixRowMajor)
            {
                m_writer.Write("%s((", prefix);
                OutputExpression(argument[1], &type1);
                m_writer.Write(")%s(", infix);
                OutputExpression(argument[0], &type0);
                m_writer.Write("))");
            }
            else
            {
                m_writer.Write("%s((", prefix);
                OutputExpression(argument[0], &type0);
                m_writer.Write(")%s(", infix);
                OutputExpression(argument[1], &type1);
                m_writer.Write("))");
            }

            handled = true;
        }
        else if (String_Equal(functionName, "saturate"))
        {
            HLSLExpression* argument[1];
            if (GetFunctionArguments(functionCall, argument, 1) != 1)
            {
                Error("saturate expects 1 argument");
                return;
            }
            m_writer.Write("clamp(");
            OutputExpression(argument[0]);
            m_writer.Write(", 0.0, 1.0)");
            handled = true;
        }

        if (!handled)
        {
            OutputIdentifier(functionName);
            m_writer.Write("(");
            OutputExpressionList(functionCall->argument, functionCall->function->argument);
            m_writer.Write(")");
        }
    }
    else
    {
        m_writer.Write("<unknown expression>");
    }


}

void GLSLGenerator::OutputIdentifier(const char* name)
{
    DepthGuard milkrustDepth;

    // Remap intrinstic functions.
    if (String_Equal(name, "tex2D"))
    {
        name = m_versionLegacy ? "texture2D" : "texture";
    }
    else if (String_Equal(name, "tex2Dproj"))
    {
        name = m_versionLegacy ? "texture2DProj" : "textureProj";
    }
    else if (String_Equal(name, "texCUBE"))
    {
        name = m_versionLegacy ? "textureCube" : "texture";
    }
    else if (String_Equal(name, "tex3D"))
    {
        name = m_versionLegacy ? "texture3D" : "texture";
    }
    else if (String_Equal(name, "clip"))
    {
        name = m_clipFunction;
    }
    else if (String_Equal(name, "tex2Dlod"))
    {
        name = m_tex2DlodFunction;
    }
    else if (String_Equal(name, "tex2Dbias"))
    {
        name = m_tex2DbiasFunction;
    }
    else if (String_Equal(name, "tex2Dgrad"))
    {
        name = m_tex2DgradFunction;
    }
    else if (String_Equal(name, "tex2DArray"))
    {
        name = "texture";
    }
    else if (String_Equal(name, "texCUBEbias"))
    {
        name = m_texCUBEbiasFunction;
    }
	else if( String_Equal( name, "texCUBElod" ) )
	{
		name = m_texCUBElodFunction;
	}
    else if (String_Equal(name, "atan2"))
    {
        name = "atan";
    }
    else if (String_Equal(name, "sincos"))
    {
        name = m_sinCosFunction;
    }
    else if (String_Equal(name, "fmod"))
    {
        name = "milkrust_fmod";
    }
    else if (String_Equal(name, "lerp"))
    {
        name = "mix";
    }
    else if (String_Equal(name, "frac"))
    {
        name = "fract";
    }
    else if (String_Equal(name, "ddx"))
    {
        name = "dFdx";
    }
    else if (String_Equal(name, "ddy"))
    {
        name = "dFdy";
    }
    else
    {
        // The identifier could be a GLSL reserved word (if it's not also a HLSL reserved word).
        name = GetSafeIdentifierName(name);
    }
    m_writer.Write("%s", name);

}

void GLSLGenerator::OutputArguments(HLSLArgument* argument)
{
    DepthGuard milkrustDepth;
    int numArgs = 0;
    while (argument != NULL)
    {
        if (numArgs > 0)
        {
            m_writer.Write(", ");
        }

        switch (argument->modifier)
        {
        case HLSLArgumentModifier_In:
            m_writer.Write("in ");
            break;
        case HLSLArgumentModifier_Out:
            m_writer.Write("out ");
            break;
        case HLSLArgumentModifier_Inout:
            m_writer.Write("inout ");
            break;
        default:
            break;
        }

        OutputDeclaration(argument->type, argument->name);
        argument = argument->nextArgument;
        ++numArgs;
    }
}

void GLSLGenerator::OutputStatements(int indent, HLSLStatement* statement, const HLSLType* returnType)
{
    DepthGuard milkrustDepth;

    while (statement != NULL)
    {
        if (statement->hidden)
        {
            statement = statement->nextStatement;
            continue;
        }

        if (statement->nodeType == HLSLNodeType_Declaration)
        {
            HLSLDeclaration* declaration = static_cast<HLSLDeclaration*>(statement);
            // MilkRust supplies this reserved marker after its host bindings.
            // User globals have no external constant-buffer provider: preserve
            // their HLSL initializers/default zero in writable fragment storage.
            if (indent == 0 && String_Equal(declaration->name, "milkrust_host_end")) {
                m_milkdropGlobals = true;
                statement = statement->nextStatement;
                continue;
            }
            // GLSL doesn't seem have texture uniforms, so just ignore them.
            for (auto* item = declaration; item; item = item->nextDeclaration)
            {
                if (item->type.baseType == HLSLBaseType_Texture) continue;
                const bool deferred = indent == 0 && !IsSamplerType(item->type.baseType) &&
                    (m_milkdropGlobals || item->assignment != NULL) && !DirectConstantDeclaration(item);
                // Grouped HLSL declarators can have different array row types.
                // Keep a separate declaration/initializer record for each one.
                auto* single = m_tree->AddNode<HLSLDeclaration>(item->fileName, item->line);
                *single = *item; single->nextDeclaration = NULL; single->nextStatement = NULL;
                m_writer.BeginLine(indent, item->fileName, item->line);
                if (indent == 0)
                {
                    // At the top level, we need the "uniform" keyword.
                    if (!deferred && (item->type.flags & (HLSLTypeFlag_Static | HLSLTypeFlag_Const)) == 0)
                        m_writer.Write("uniform ");
                }
                OutputDeclaration(single, deferred);
                m_writer.EndLine(";");
                if (deferred) m_globalInitializers.push_back(single);
            }
        }
        else if (statement->nodeType == HLSLNodeType_Struct)
        {
            // Emitted with its dependencies before helper functions.
        }
        else if (statement->nodeType == HLSLNodeType_Buffer)
        {
            HLSLBuffer* buffer = static_cast<HLSLBuffer*>(statement);
            OutputBuffer(indent, buffer);
        }
        else if (statement->nodeType == HLSLNodeType_Function)
        {
            HLSLFunction* function = static_cast<HLSLFunction*>(statement);

            // Use an alternate name for the function which is supposed to be entry point
            // so that we can supply our own function which will be the actual entry point.
            const char* functionName   = GetSafeIdentifierName(function->name);
            const char* returnTypeName = GetTypeName(function->returnType);

            m_writer.BeginLine(indent, function->fileName, function->line);
            m_writer.Write("%s", returnTypeName);
            if (function->returnType.array)
                m_writer.Write("[%d]", AggregateArrayExtent(*m_tree, function->returnType));
            m_writer.Write(" %s(", functionName);

            OutputArguments(function->argument);

            if (function->forward)
            {
                m_writer.WriteLine(indent, ");");
            }
            else
            {
                m_writer.Write(") {");
                m_writer.EndLine();

                m_insideFunction = true;
                m_localTemporaries.clear();
                const size_t temporaryOffset = strlen(m_writer.GetResult());
                OutputStatements(indent + 1, function->statement, &function->returnType);
                InsertLocalTemporaries(temporaryOffset);
                m_writer.WriteLine(indent, "}");
            }
        }
        else if (statement->nodeType == HLSLNodeType_ExpressionStatement)
        {
            HLSLExpressionStatement* expressionStatement = static_cast<HLSLExpressionStatement*>(statement);
            m_writer.BeginLine(indent, statement->fileName, statement->line);
            OutputExpression(expressionStatement->expression);
            m_writer.EndLine(";");
        }
        else if (statement->nodeType == HLSLNodeType_ReturnStatement)
        {
            HLSLReturnStatement* returnStatement = static_cast<HLSLReturnStatement*>(statement);
            if (returnStatement->expression != NULL)
            {
                m_writer.BeginLine(indent, returnStatement->fileName, returnStatement->line);
                m_writer.Write("return ");
                OutputExpression(returnStatement->expression, returnType);
                m_writer.EndLine(";");
            }
            else
            {
                m_writer.WriteLineTagged(indent, returnStatement->fileName, returnStatement->line, "return;");
            }
        }
        else if (statement->nodeType == HLSLNodeType_DiscardStatement)
        {
            HLSLDiscardStatement* discardStatement = static_cast<HLSLDiscardStatement*>(statement);
            if (m_target == Target_FragmentShader)
            {
                m_writer.WriteLineTagged(indent, discardStatement->fileName, discardStatement->line, "discard;");
            }
        }
        else if (statement->nodeType == HLSLNodeType_BreakStatement)
        {
            HLSLBreakStatement* breakStatement = static_cast<HLSLBreakStatement*>(statement);
            m_writer.WriteLineTagged(indent, breakStatement->fileName, breakStatement->line, "break;");
        }
        else if (statement->nodeType == HLSLNodeType_ContinueStatement)
        {
            HLSLContinueStatement* continueStatement = static_cast<HLSLContinueStatement*>(statement);
            m_writer.WriteLineTagged(indent, continueStatement->fileName, continueStatement->line, "continue;");
        }
        else if (statement->nodeType == HLSLNodeType_IfStatement)
        {
            HLSLIfStatement* ifStatement = static_cast<HLSLIfStatement*>(statement);
            m_writer.BeginLine(indent, ifStatement->fileName, ifStatement->line);
            m_writer.Write("if (");
            OutputExpression(ifStatement->condition, &kBoolType);
            m_writer.Write(") {");
            m_writer.EndLine();
            OutputStatements(indent + 1, ifStatement->statement, returnType);
            m_writer.WriteLine(indent, "}");
            if (ifStatement->elseStatement != NULL)
            {
                m_writer.WriteLine(indent, "else {");
                OutputStatements(indent + 1, ifStatement->elseStatement, returnType);
                m_writer.WriteLine(indent, "}");
            }
        }
        else if (statement->nodeType == HLSLNodeType_BlockStatement)
        {
            HLSLBlockStatement* block = static_cast<HLSLBlockStatement*>(statement);
            m_writer.WriteLine(indent, "{");
            OutputStatements(indent + 1, block->statement, returnType);
            m_writer.WriteLine(indent, "}");
        }
        else if (statement->nodeType == HLSLNodeType_ForStatement)
        {
            HLSLForStatement* loop = static_cast<HLSLForStatement*>(statement);
            if (loop->initialization) {
                // GLSL cannot mix distinct row-wrapper types in one for
                // declarator. Preserve the loop's scope around separate locals.
                m_writer.WriteLine(indent, "{");
                for (auto* item = loop->initialization; item; item = item->nextDeclaration) {
                    HLSLDeclaration single = *item; single.nextDeclaration = NULL;
                    m_writer.BeginLine(indent + 1); OutputDeclaration(&single); m_writer.EndLine(";");
                }
                ++indent;
            }
            m_writer.BeginLine(indent, loop->fileName, loop->line);
            if (loop->postTest) m_writer.Write("do {");
            else {
                m_writer.Write("for (");
                if (loop->initializationExpression) OutputExpression(loop->initializationExpression);
                m_writer.Write("; ");
                if (loop->condition) OutputExpression(loop->condition, &kBoolType);
                m_writer.Write("; ");
                if (loop->increment) OutputExpression(loop->increment);
                m_writer.Write(") {");
            }
            m_writer.EndLine();
            m_writer.WriteLine(indent + 1, "if (--milkrust_loop_budget < 0) discard;");
            OutputStatements(indent + 1, loop->statement, returnType);
            if (loop->postTest) {
                m_writer.BeginLine(indent); m_writer.Write("} while (");
                OutputExpression(loop->condition, &kBoolType); m_writer.EndLine(");");
            } else m_writer.WriteLine(indent, "}");
            if (loop->initialization) m_writer.WriteLine(--indent, "}");
        }
        else if (statement->nodeType == HLSLNodeType_SwitchStatement) {
            HLSLSwitchStatement* node = static_cast<HLSLSwitchStatement*>(statement);
            m_writer.BeginLine(indent); m_writer.Write("switch (");
            OutputExpression(node->condition); m_writer.EndLine(") {");
            OutputStatements(indent + 1, node->statement, returnType);
            m_writer.WriteLine(indent, "}");
        }
        else if (statement->nodeType == HLSLNodeType_CaseStatement) {
            HLSLCaseStatement* node = static_cast<HLSLCaseStatement*>(statement);
            m_writer.BeginLine(indent);
            if (node->expression) { m_writer.Write("case "); OutputExpression(node->expression); }
            else m_writer.Write("default");
            m_writer.EndLine(":");
        }
        else
        {
            Error("Unsupported HLSL statement node");
        }

        statement = statement->nextStatement;

    }

}

void GLSLGenerator::OutputBuffer(int indent, HLSLBuffer* buffer)
{
    DepthGuard milkrustDepth;
    // Empty uniform blocks cause compilation errors on NVIDIA, so don't emit them.
    if (buffer->field == NULL)
        return;

    if (m_options.flags & Flag_EmulateConstantBuffer)
    {
        unsigned int size = 0;
        LayoutBuffer(buffer, size);

        unsigned int uniformSize = (size + 3) / 4;

        m_writer.WriteLineTagged(indent, buffer->fileName, buffer->line, "uniform vec4 %s%s[%d];", m_options.constantBufferPrefix, buffer->name, uniformSize);
    }
    else
    {
        m_writer.WriteLineTagged(indent, buffer->fileName, buffer->line, "layout (std140) uniform %s%s {", m_options.constantBufferPrefix, buffer->name);
        HLSLDeclaration* field = buffer->field;
        while (field != NULL)
        {
            m_writer.BeginLine(indent + 1, field->fileName, field->line);
            OutputDeclaration(field->type, field->name);
            m_writer.Write(";");
            m_writer.EndLine();
            field = (HLSLDeclaration*)field->nextStatement;
        }
        m_writer.WriteLine(indent, "};");
    }
}

inline void alignForWrite(unsigned int& offset, unsigned int size)
{
    ASSERT(size <= 4);

    if (offset / 4 != (offset + size - 1) / 4)
        offset = (offset + 3) & ~3;
}

void GLSLGenerator::LayoutBuffer(HLSLBuffer* buffer, unsigned int& offset)
{
    for (HLSLDeclaration* field = buffer->field; field; field = (HLSLDeclaration*)field->nextStatement)
    {
        LayoutBuffer(field->type, offset);
    }
}

void GLSLGenerator::LayoutBuffer(const HLSLType& type, unsigned int& offset)
{
    if (type.array)
    {
        const int arraySize = AggregateArrayExtent(*m_tree, type);
        LayoutBufferAlign(type, offset);
        HLSLType element = type; element.DropArrayDimension();
        unsigned int elementSize = 0;
        LayoutBuffer(element, elementSize);
        const unsigned int stride = (elementSize + 3) & ~3;
        offset += stride * static_cast<unsigned int>(arraySize);
    }
    else
    {
        LayoutBufferAlign(type, offset);
        LayoutBufferElement(type, offset);
    }
}

void GLSLGenerator::LayoutBufferElement(const HLSLType& type, unsigned int& offset)
{
    if (type.baseType == HLSLBaseType_Float)
    {
        offset += 1;
    }
    else if (type.baseType == HLSLBaseType_Float2)
    {
        offset += 2;
    }
    else if (type.baseType == HLSLBaseType_Float3)
    {
        offset += 3;
    }
    else if (type.baseType == HLSLBaseType_Float4)
    {
        offset += 4;
    }
    else if (type.baseType == HLSLBaseType_Float4x4)
    {
        offset += 16;
    }
    else if (type.baseType == HLSLBaseType_UserDefined)
    {
        HLSLStruct * st = m_tree->FindGlobalStruct(type.typeName);

        if (st)
        {
            for (HLSLStructField* field = st->field; field; field = field->nextField)
            {
                LayoutBuffer(field->type, offset);
            }
        }
        else
        {
            Error("Unknown type %s", type.typeName);
        }
    }
    else
    {
        Error("Constant buffer layout is not supported for %s", GetTypeName(type));
    }
}

void GLSLGenerator::LayoutBufferAlign(const HLSLType& type, unsigned int& offset)
{
    if (type.array)
    {
        alignForWrite(offset, 4);
    }
    else if (type.baseType == HLSLBaseType_Float)
    {
        alignForWrite(offset, 1);
    }
    else if (type.baseType == HLSLBaseType_Float2)
    {
        alignForWrite(offset, 2);
    }
    else if (type.baseType == HLSLBaseType_Float3)
    {
        alignForWrite(offset, 3);
    }
    else if (type.baseType == HLSLBaseType_Float4)
    {
        alignForWrite(offset, 4);
    }
    else if (type.baseType == HLSLBaseType_Float4x4)
    {
        alignForWrite(offset, 4);
    }
    else if (type.baseType == HLSLBaseType_UserDefined)
    {
        alignForWrite(offset, 4);
    }
    else
    {
        Error("Constant buffer layout is not supported for %s", GetTypeName(type));
    }
}

HLSLBuffer* GLSLGenerator::GetBufferAccessExpression(HLSLExpression* expression)
{
    if (expression->nodeType == HLSLNodeType_IdentifierExpression)
    {
        HLSLIdentifierExpression* identifierExpression = static_cast<HLSLIdentifierExpression*>(expression);

        if (identifierExpression->global)
        {
            HLSLDeclaration * declaration = m_tree->FindGlobalDeclaration(identifierExpression->name);

            if (declaration && declaration->buffer)
                return declaration->buffer;
        }
    }
    else if (expression->nodeType == HLSLNodeType_MemberAccess)
    {
        HLSLMemberAccess* memberAccess = static_cast<HLSLMemberAccess*>(expression);

        if (memberAccess->object->expressionType.baseType == HLSLBaseType_UserDefined)
            return GetBufferAccessExpression(memberAccess->object);
    }
    else if (expression->nodeType == HLSLNodeType_ArrayAccess)
    {
        HLSLArrayAccess* arrayAccess = static_cast<HLSLArrayAccess*>(expression);

        if (arrayAccess->array->expressionType.array)
            return GetBufferAccessExpression(arrayAccess->array);
    }

    return 0;
}

void GLSLGenerator::OutputBufferAccessExpression(HLSLBuffer* buffer, HLSLExpression* expression, const HLSLType& type, unsigned int postOffset)
{
    DepthGuard milkrustDepth;
    if (type.array)
    {
        Error("Constant buffer access is not supported for arrays (use indexing instead)");
    }
    else if (type.baseType == HLSLBaseType_Float)
    {
        m_writer.Write("%s%s[", m_options.constantBufferPrefix, buffer->name);
        unsigned int index = OutputBufferAccessIndex(expression, postOffset);
        m_writer.Write("%d].%c", index / 4, "xyzw"[index % 4]);
    }
    else if (type.baseType == HLSLBaseType_Float2)
    {
        m_writer.Write("%s%s[", m_options.constantBufferPrefix, buffer->name);
        unsigned int index = OutputBufferAccessIndex(expression, postOffset);
        m_writer.Write("%d].%s", index / 4, index % 4 == 0 ? "xy" : index % 4 == 1 ? "yz" : "zw");
    }
    else if (type.baseType == HLSLBaseType_Float3)
    {
        m_writer.Write("%s%s[", m_options.constantBufferPrefix, buffer->name);
        unsigned int index = OutputBufferAccessIndex(expression, postOffset);
        m_writer.Write("%d].%s", index / 4, index % 4 == 0 ? "xyz" : "yzw");
    }
    else if (type.baseType == HLSLBaseType_Float4)
    {
        m_writer.Write("%s%s[", m_options.constantBufferPrefix, buffer->name);
        unsigned int index = OutputBufferAccessIndex(expression, postOffset);
        ASSERT(index % 4 == 0);
        m_writer.Write("%d]", index / 4);
    }
    else if (type.baseType == HLSLBaseType_Float4x4)
    {
        m_writer.Write("mat4(");
        for (int i = 0; i < 4; ++i)
        {
            m_writer.Write("%s%s[", m_options.constantBufferPrefix, buffer->name);
            unsigned int index = OutputBufferAccessIndex(expression, postOffset + i * 4);
            ASSERT(index % 4 == 0);
            m_writer.Write("%d]%c", index / 4, i == 3 ? ')' : ',');
        }
    }
    else if (type.baseType == HLSLBaseType_UserDefined)
    {
        HLSLStruct * st = m_tree->FindGlobalStruct(type.typeName);

        if (st)
        {
            m_writer.Write("%s(", st->name);

            unsigned int offset = postOffset;

            for (HLSLStructField* field = st->field; field; field = field->nextField)
            {
                OutputBufferAccessExpression(buffer, expression, field->type, offset);

                if (field->nextField)
                    m_writer.Write(",");

                LayoutBuffer(field->type, offset);
            }

            m_writer.Write(")");
        }
        else
        {
            Error("Unknown type %s", type.typeName);
        }
    }
    else
    {
        Error("Constant buffer layout is not supported for %s", GetTypeName(type));
    }
}

unsigned int GLSLGenerator::OutputBufferAccessIndex(HLSLExpression* expression, unsigned int postOffset)
{
    DepthGuard milkrustDepth;
    if (expression->nodeType == HLSLNodeType_IdentifierExpression)
    {
        HLSLIdentifierExpression* identifierExpression = static_cast<HLSLIdentifierExpression*>(expression);
        ASSERT(identifierExpression->global);

        HLSLDeclaration * declaration = m_tree->FindGlobalDeclaration(identifierExpression->name);
        ASSERT(declaration);

        HLSLBuffer * buffer = declaration->buffer;
        ASSERT(buffer);

        unsigned int offset = 0;

        for (HLSLDeclaration* field = buffer->field; field; field = (HLSLDeclaration*)field->nextStatement)
        {
            if (field == declaration)
            {
                LayoutBufferAlign(field->type, offset);
                break;
            }

            LayoutBuffer(field->type, offset);
        }

        return offset + postOffset;
    }
    else if (expression->nodeType == HLSLNodeType_MemberAccess)
    {
        HLSLMemberAccess* memberAccess = static_cast<HLSLMemberAccess*>(expression);

        const HLSLType& type = memberAccess->object->expressionType;
        ASSERT(type.baseType == HLSLBaseType_UserDefined);

        HLSLStruct * st = m_tree->FindGlobalStruct(type.typeName);

        if (st)
        {
            unsigned int offset = 0;

            for (HLSLStructField* field = st->field; field; field = field->nextField)
            {
                if (field->name == memberAccess->field)
                {
                    LayoutBufferAlign(field->type, offset);
                    break;
                }

                LayoutBuffer(field->type, offset);
            }

            return offset + OutputBufferAccessIndex(memberAccess->object, postOffset);
        }
        else
        {
            Error("Unknown type %s", type.typeName);
        }
    }
    else if (expression->nodeType == HLSLNodeType_ArrayAccess)
    {
        HLSLArrayAccess* arrayAccess = static_cast<HLSLArrayAccess*>(expression);

        const HLSLType& type = arrayAccess->array->expressionType;
        ASSERT(type.array);
        HLSLType element = type; element.DropArrayDimension();
        unsigned int elementSize = 0;
        LayoutBuffer(element, elementSize);
        const unsigned int alignedElementSize = (elementSize + 3) & ~3;

        int arrayIndex = 0;
        if (m_tree->GetExpressionValue(arrayAccess->index, arrayIndex))
        {
            unsigned int offset = arrayIndex * alignedElementSize;

            return offset + OutputBufferAccessIndex(arrayAccess->array, postOffset);
        }
        else
        {
            m_writer.Write("%d*(", alignedElementSize / 4);
            OutputExpression(arrayAccess->index);
            m_writer.Write(")+");

            return OutputBufferAccessIndex(arrayAccess->array, postOffset);
        }
    }
    else
    {
        ASSERT(!"IsBufferAccessExpression should have returned false");
    }

    return 0;
}

HLSLFunction* GLSLGenerator::FindFunction(HLSLRoot* root, const char* name)
{
    HLSLStatement* statement = root->statement;
    while (statement != NULL)
    {
        if (statement->nodeType == HLSLNodeType_Function)
        {
            HLSLFunction* function = static_cast<HLSLFunction*>(statement);
            if (String_Equal(function->name, name))
            {
                return function;
            }
        }
        statement = statement->nextStatement;
    }
    return NULL;
}

HLSLStruct* GLSLGenerator::FindStruct(HLSLRoot* root, const char* name)
{
    HLSLStatement* statement = root->statement;
    while (statement != NULL)
    {
        if (statement->nodeType == HLSLNodeType_Struct)
        {
            HLSLStruct* structDeclaration = static_cast<HLSLStruct*>(statement);
            if (String_Equal(structDeclaration->name, name))
            {
                return structDeclaration;
            }
        }
        statement = statement->nextStatement;
    }
    return NULL;
}


const char* GLSLGenerator::GetAttribQualifier(AttributeModifier modifier)
{
    if (m_versionLegacy)
    {
        if (m_target == Target_VertexShader)
            return (modifier == AttributeModifier_In) ? "attribute" : "varying";
        else
            return (modifier == AttributeModifier_In) ? "varying" : "out";
    }
    else
    {
        return (modifier == AttributeModifier_In) ? "in" : "out";
    }
}

void GLSLGenerator::OutputAttribute(const HLSLType& type, const char* semantic, AttributeModifier modifier)
{
    DepthGuard milkrustDepth;
    const char* qualifier = GetAttribQualifier(modifier);
    const char* prefix = (modifier == AttributeModifier_In) ? m_inAttribPrefix : m_outAttribPrefix;

    HLSLRoot* root = m_tree->GetRoot();
    if (type.baseType == HLSLBaseType_UserDefined)
    {
        // If the argument is a struct with semantics specified, we need to
        // grab them.
        HLSLStruct* structDeclaration = FindStruct(root, type.typeName);
        ASSERT(structDeclaration != NULL);
        HLSLStructField* field = structDeclaration->field;
        while (field != NULL)
        {
            if (field->semantic != NULL && GetBuiltInSemantic(field->semantic, modifier) == NULL)
            {
                m_writer.Write( "%s ", qualifier );
				char attribName[ 64 ];
				String_Printf( attribName, 64, "%s%s", prefix, field->semantic );
				OutputDeclaration( field->type, attribName );
				m_writer.EndLine(";");
            }
            field = field->nextField;
        }
    }
    else if (semantic != NULL && GetBuiltInSemantic(semantic, modifier) == NULL)
    {
		m_writer.Write( "%s ", qualifier );
		char attribName[ 64 ];
		String_Printf( attribName, 64, "%s%s", prefix, semantic );
		OutputDeclaration( type, attribName );
		m_writer.EndLine(";");
    }
}

void GLSLGenerator::OutputAttributes(HLSLFunction* entryFunction)
{
    DepthGuard milkrustDepth;
    // Write out the input/output attributes to the shader.
    HLSLArgument* argument = entryFunction->argument;
    while (argument != NULL)
    {
        if (argument->modifier == HLSLArgumentModifier_None || argument->modifier == HLSLArgumentModifier_In)
            OutputAttribute(argument->type, argument->semantic, AttributeModifier_In);
        if (argument->modifier == HLSLArgumentModifier_Out)
            OutputAttribute(argument->type, argument->semantic, AttributeModifier_Out);

        argument = argument->nextArgument;
    }

    // Write out the output attributes from the shader.
    OutputAttribute(entryFunction->returnType, entryFunction->semantic, AttributeModifier_Out);
}

void GLSLGenerator::OutputSetOutAttribute(const char* semantic, const char* resultName)
{
    DepthGuard milkrustDepth;
    int outputIndex = -1;
    const char* builtInSemantic = GetBuiltInSemantic(semantic, AttributeModifier_Out, &outputIndex);
    if (builtInSemantic != NULL)
    {
        if (String_Equal(builtInSemantic, "gl_Position"))
        {
            if (m_options.flags & Flag_FlipPositionOutput)
            {
                // Mirror the y-coordinate when we're outputing from
                // the vertex shader so that we match the D3D texture
                // coordinate origin convention in render-to-texture
                // operations.
                // We also need to convert the normalized device
                // coordinates from the D3D convention of 0 to 1 to the
                // OpenGL convention of -1 to 1.
                m_writer.WriteLine(1, "vec4 temp = %s;", resultName);
                m_writer.WriteLine(1, "%s = temp * vec4(1,-1,2,1) - vec4(0,0,temp.w,0);", builtInSemantic);
            }
            else
            {
                m_writer.WriteLine(1, "%s = %s;", builtInSemantic, resultName);
            }

            m_outputPosition = true;
        }
        else if (String_Equal(builtInSemantic, "gl_FragDepth"))
        {
            // If the value goes outside of the 0 to 1 range, the
            // fragment will be rejected unlike in D3D, so clamp it.
            m_writer.WriteLine(1, "%s = clamp(float(%s), 0.0, 1.0);", builtInSemantic, resultName);
        }
        else if (outputIndex >= 0)
        {
            m_writer.WriteLine(1, "%s[%d] = %s;", builtInSemantic, outputIndex, resultName);
        }
        else
        {
            m_writer.WriteLine(1, "%s = %s;", builtInSemantic, resultName);
        }
    }
    else if (m_target == Target_FragmentShader)
    {
        Error("Output attribute %s does not map to any built-ins", semantic);
    }
    else
    {
        m_writer.WriteLine(1, "%s%s = %s;", m_outAttribPrefix, semantic, resultName);
    }
}

void GLSLGenerator::OutputEntryCaller(HLSLFunction* entryFunction)
{
    DepthGuard milkrustDepth;
    HLSLRoot* root = m_tree->GetRoot();

    m_writer.WriteLine(0, "void main() {");
    m_insideFunction = true;
    m_localTemporaries.clear();
    const size_t temporaryOffset = strlen(m_writer.GetResult());
    m_writer.WriteLine(1, "milkrust_loop_budget = 16384;");
    for (auto* first : m_globalInitializers) {
        for (auto* declaration = first; declaration; declaration = declaration->nextDeclaration) {
            CheckBudget();
            m_writer.BeginLine(1);
            m_writer.Write("%s = ", GetSafeIdentifierName(declaration->name));
            if (declaration->assignment) OutputExpression(declaration->assignment, &declaration->type);
            else OutputDefaultValue(declaration->type);
            m_writer.EndLine(";");
        }
    }


    // Create local variables for each of the parameters we'll need to pass
    // into the entry point function.
    HLSLArgument* argument = entryFunction->argument;
    while (argument != NULL)
    {
        m_writer.BeginLine(1);
        OutputDeclaration(argument->type, argument->name);
        m_writer.EndLine(";");

        if (argument->modifier != HLSLArgumentModifier_Out)
        {
            // Set the value for the local variable.
            if (argument->type.baseType == HLSLBaseType_UserDefined)
            {
                HLSLStruct* structDeclaration = FindStruct(root, argument->type.typeName);
                ASSERT(structDeclaration != NULL);
                HLSLStructField* field = structDeclaration->field;
                while (field != NULL)
                {
                    if (field->semantic != NULL)
                    {
                        const char* builtInSemantic = GetBuiltInSemantic(field->semantic, AttributeModifier_In);
                        if (builtInSemantic)
                        {
                            m_writer.WriteLine(1, "%s.%s = %s;", GetSafeIdentifierName(argument->name), GetSafeIdentifierName(field->name), builtInSemantic);
                        }
                        else
                        {
                            m_writer.WriteLine(1, "%s.%s = %s%s;", GetSafeIdentifierName(argument->name), GetSafeIdentifierName(field->name), m_inAttribPrefix, field->semantic);
                        }
                    }
                    field = field->nextField;
                }
            }
            else if (argument->semantic != NULL)
            {
                const char* builtInSemantic = GetBuiltInSemantic(argument->semantic, AttributeModifier_In);
                if (builtInSemantic)
                {
                    m_writer.WriteLine(1, "%s = %s;", GetSafeIdentifierName(argument->name), builtInSemantic);
                }
                else
                {
                    m_writer.WriteLine(1, "%s = %s%s;", GetSafeIdentifierName(argument->name), m_inAttribPrefix, argument->semantic);
                }
            }
        }

        argument = argument->nextArgument;
    }

    const char* resultName = "result";

    // Call the original entry function.
    m_writer.BeginLine(1);
    if (entryFunction->returnType.baseType != HLSLBaseType_Void)
        m_writer.Write("%s %s = ", GetTypeName(entryFunction->returnType), resultName);
    m_writer.Write("%s(", m_entryName);

    int numArgs = 0;
    argument = entryFunction->argument;
    while (argument != NULL)
    {
        if (numArgs > 0)
        {
            m_writer.Write(", ");
        }

        m_writer.Write("%s", GetSafeIdentifierName(argument->name));

        argument = argument->nextArgument;
        ++numArgs;
    }
    m_writer.EndLine(");");

    // Copy values from the result into the out attributes as necessary.
    argument = entryFunction->argument;
    while (argument != NULL)
    {
        if (argument->modifier == HLSLArgumentModifier_Out && argument->semantic)
            OutputSetOutAttribute(argument->semantic, GetSafeIdentifierName(argument->name));

        argument = argument->nextArgument;
    }

    if (entryFunction->returnType.baseType == HLSLBaseType_UserDefined)
    {
        HLSLStruct* structDeclaration = FindStruct(root, entryFunction->returnType.typeName);
        ASSERT(structDeclaration != NULL);
        HLSLStructField* field = structDeclaration->field;
        while (field != NULL)
        {
            char fieldResultName[1024];
            String_Printf( fieldResultName, sizeof(fieldResultName), "%s.%s", resultName, field->name );
            OutputSetOutAttribute( field->semantic, fieldResultName );
            field = field->nextField;
        }
    }
    else if (entryFunction->semantic != NULL)
    {
        OutputSetOutAttribute(entryFunction->semantic, resultName);
    }

    InsertLocalTemporaries(temporaryOffset);
    m_writer.WriteLine(0, "}");
}

void GLSLGenerator::OutputDeclaration(HLSLDeclaration* declaration, bool skipInitializer)
{
    DepthGuard milkrustDepth;
    HLSLType declaredType = declaration->type;
    // Preserve provable integer constants used as array bounds. HLSL also
    // permits local const values initialized from uniforms; GLSL ES const does
    // not, so do not blindly add the GLSL qualifier to every readonly value.
    bool constantIntegers = declaredType.baseType == HLSLBaseType_Int &&
        (declaredType.flags & HLSLTypeFlag_Const) && !declaredType.array && !skipInitializer;
    for (auto* item=declaration; constantIntegers && item; item=item->nextDeclaration) {
        int value=0;
        constantIntegers = item->assignment && m_tree->GetExpressionValue(item->assignment, value);
    }
    bool directConstant = !skipInitializer;
    for (auto* item = declaration; directConstant && item; item = item->nextDeclaration)
        directConstant = DirectConstantDeclaration(item);
    declaredType.flags &= ~(HLSLTypeFlag_Const | HLSLTypeFlag_Static);
    if (directConstant || constantIntegers) declaredType.flags |= HLSLTypeFlag_Const | HLSLTypeFlag_Static;
    OutputDeclarationType(declaredType);

	HLSLDeclaration* lastDecl = nullptr;
	while( declaration )
	{
		if( lastDecl )
			m_writer.Write( ", " );

		OutputDeclarationBody( declaration->type, GetSafeIdentifierName( declaration->name ) );

		if( !skipInitializer && declaration->assignment != NULL )
		{
			m_writer.Write( " = " );
            if (constantIntegers) {
                int value=0; m_tree->GetExpressionValue(declaration->assignment, value);
                m_writer.Write("%d", value);
            }

			else
			{
				OutputExpression( declaration->assignment, &declaration->type );
			}
		}

		lastDecl = declaration;
		declaration = declaration->nextDeclaration;
	}
}

void GLSLGenerator::OutputTypeDefinition(const HLSLType& type)
{
    DepthGuard depth;
    if (type.arrayRank > 1) {
        const std::string name = GetTypeName(type);
        const std::string key = "array:" + name;
        if (m_typeDefinitions.count(key)) return;
        HLSLType element = type; element.DropArrayDimension();
        element.flags = 0;
        OutputTypeDefinition(element);
        m_writer.WriteLine(0, "struct %s {", name.c_str());
        m_writer.BeginLine(1); OutputDeclaration(element, "elements"); m_writer.EndLine(";");
        m_writer.WriteLine(0, "};");
        m_typeDefinitions.insert(key);
    } else if (type.baseType == HLSLBaseType_UserDefined) {
        const std::string key = std::string("struct:") + type.typeName;
        if (m_typeDefinitions.count(key)) return;
        if (!m_pendingTypeDefinitions.insert(key).second)
            throw std::invalid_argument("Recursive HLSL structure layout");
        auto* structure = m_tree->FindGlobalStruct(type.typeName);
        if (!structure) throw std::invalid_argument("Unknown HLSL structure layout");
        for (auto* field = structure->field; field; field = field->nextField)
            OutputTypeDefinition(field->type);
        m_writer.WriteLine(0, "struct %s {", structure->name);
        for (auto* field = structure->field; field; field = field->nextField) {
            m_writer.BeginLine(1); OutputDeclaration(field->type, field->name); m_writer.EndLine(";");
        }
        m_writer.WriteLine(0, "};");
        m_pendingTypeDefinitions.erase(key);
        m_typeDefinitions.insert(key);
    }
}

void GLSLGenerator::OutputDefaultValue(const HLSLType& type)
{
    DepthGuard milkrustDepth;
    if (type.array) {
        const int count = AggregateArrayExtent(*m_tree, type);
        HLSLType element = type; element.DropArrayDimension();
        m_writer.Write("%s[](", GetTypeName(type));
        for (int i=0; i<count; ++i) {
            CheckBudget(); if (i) m_writer.Write(", ");
            if (type.arrayRank > 1) m_writer.Write("%s(", GetTypeName(type));
            OutputDefaultValue(element);
            if (type.arrayRank > 1) m_writer.Write(")");
        }
        m_writer.Write(")");
    } else if (type.baseType == HLSLBaseType_UserDefined) {
        auto* structure = FindStruct(m_tree->GetRoot(), type.typeName);
        if (!structure) { Error("Unknown global structure"); return; }
        m_writer.Write("%s(", type.typeName);
        for (auto* field=structure->field; field; field=field->nextField) {
            CheckBudget(); OutputDefaultValue(field->type); if (field->nextField) m_writer.Write(", ");
        }
        m_writer.Write(")");
    } else if (IsCustomMatrixType(type.baseType)) {
        const int rows = MatrixRows(type.baseType), columns = NumericWidth(type.baseType);
        const HLSLType rowType(NumericShape(ScalarBaseType[type.baseType], 0, columns));
        m_writer.Write("%s(", GetTypeName(type));
        for (int row = 0; row < rows; ++row) {
            if (row) m_writer.Write(", ");
            m_writer.Write("%s(0)", GetTypeName(rowType));
        }
        m_writer.Write(")");
    } else m_writer.Write("%s(0)", GetTypeName(type));
}

void GLSLGenerator::OutputDeclaration(const HLSLType& type, const char* name)
{
    DepthGuard milkrustDepth;
	OutputDeclarationType( type );
	OutputDeclarationBody( type, name );
}

void GLSLGenerator::OutputDeclarationType( const HLSLType& type )
{
    DepthGuard milkrustDepth;
	if ((type.flags & HLSLTypeFlag_Const) && (type.flags & HLSLTypeFlag_Static))
	{
		m_writer.Write("const ");
	}

	m_writer.Write( "%s ", GetTypeName( type ) );
}

void GLSLGenerator::OutputDeclarationBody( const HLSLType& type, const char* name )
{
    DepthGuard milkrustDepth;
	if( !type.array )
	{
		m_writer.Write( "%s", GetSafeIdentifierName( name ) );
	}
	else
	{
		m_writer.Write( "%s", GetSafeIdentifierName( name ) );
        for (int i = 0; i < std::min(type.arrayRank, 1); ++i) {
			m_writer.Write("[");
			if (type.arraySizes[i] != NULL) {
				int extent = 0;
				if (!m_tree->GetExpressionValue(type.arraySizes[i], extent) || extent < 1 || extent > 4096) {
					Error("Invalid constant array extent"); return;
				}
				m_writer.Write("%d", extent);
			}
			m_writer.Write("]");
		}
	}
}

void GLSLGenerator::OutputCast(const HLSLType& type)
{
    DepthGuard milkrustDepth;
    if (m_version == Version_110 && type.baseType == HLSLBaseType_Float3x3)
        m_writer.Write("%s", m_matrixCtorFunction);
    else
        OutputDeclaration(type, "");
}

void GLSLGenerator::Error(const char* format, ...)
{
    // It's not always convenient to stop executing when an error occurs,
    // so just track once we've hit an error and stop reporting them until
    // we successfully bail out of execution.
    if (m_error)
    {
        return;
    }
    m_error = true;

    va_list arg;
    va_start(arg, format);
    Log_ErrorArgList(format, arg);
    va_end(arg);
}

const char* GLSLGenerator::GetSafeIdentifierName(const char* name) const
{
    for (int i = 0; i < s_numReservedWords; ++i)
    {
        if (String_Equal(s_reservedWord[i], name))
        {
            return m_reservedWord[i];
        }
    }
    return name;
}

bool GLSLGenerator::ChooseUniqueName(const char* base, char* dst, int dstLength) const
{
    for (int i = 0; i < 1024; ++i)
    {
        String_Printf(dst, dstLength, "%s%d", base, i);
        if (!m_tree->GetContainsString(dst))
        {
            return true;
        }
    }
    return false;
}

const char* GLSLGenerator::GetBuiltInSemantic(const char* semantic, AttributeModifier modifier, int* outputIndex)
{
    if (outputIndex)
        *outputIndex = -1;

    if (m_target == Target_VertexShader && modifier == AttributeModifier_Out && String_Equal(semantic, "POSITION"))
        return "gl_Position";

    if (m_target == Target_VertexShader && modifier == AttributeModifier_Out && String_Equal(semantic, "SV_Position"))
        return "gl_Position";

    if (m_target == Target_VertexShader && modifier == AttributeModifier_Out && String_Equal(semantic, "PSIZE"))
        return "gl_PointSize";

    if (m_target == Target_VertexShader && modifier == AttributeModifier_In && String_Equal(semantic, "SV_InstanceID"))
        return "gl_InstanceID";

    if (m_target == Target_FragmentShader && modifier == AttributeModifier_Out && String_Equal(semantic, "SV_Depth"))
        return "gl_FragDepth";

    if (m_target == Target_FragmentShader && modifier == AttributeModifier_In && String_Equal(semantic, "SV_Position"))
        return "gl_FragCoord";

    if (m_target == Target_FragmentShader && modifier == AttributeModifier_Out)
    {
        int index = -1;

        if (strncmp(semantic, "COLOR", 5) == 0)
            index = atoi(semantic + 5);
        else if (strncmp(semantic, "SV_Target", 9) == 0)
            index = atoi(semantic + 9);

        if (index >= 0)
        {
            if (m_outputTargets <= index)
                m_outputTargets = index + 1;

            if (outputIndex)
                *outputIndex = index;

            return m_versionLegacy ? "gl_FragData" : "rast_FragData";
        }
    }

    return NULL;
}

}
