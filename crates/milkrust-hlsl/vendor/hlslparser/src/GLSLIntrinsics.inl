// Legacy HLSL intrinsics whose signatures or arithmetic differ from GLSL ES.
// Helpers are emitted only when used and stay inside the compiler's budgets.
bool GLSLGenerator::OutputLegacyIntrinsic(HLSLFunctionCall* expression) {
    const std::string function = expression->function->name;
    const auto* first = expression->function->argument;
    if (!first) return false;
    const auto base = first->type.baseType;
    const int rows = MatrixRows(base), width = NumericWidth(base);
    const std::string type = GetTypeName(first->type);
    const std::string resultType = GetTypeName(expression->expressionType);
    if (function == "tex2Dlod" && expression->function->numArguments == 3) {
        auto* sampler = expression->argument;
        auto* coordinate = sampler->nextExpression;
        auto* offset = coordinate->nextExpression;
        auto* temporary = LocalTemporary(first->nextArgument->type);
        m_writer.Write("(%s = ", temporary->name);
        OutputExpression(coordinate, &temporary->expressionType);
        m_writer.Write(", textureLodOffset("); OutputExpression(sampler);
        m_writer.Write(", %s.xy, %s.w, ", temporary->name, temporary->name);
        // GLSL requires the offset to remain a constant expression. A helper
        // parameter would destroy constness, so emit the original expression.
        OutputExpression(offset, &first->nextArgument->nextArgument->type);
        m_writer.Write("))"); return true;
    }
    std::string parameters;
    int count = 0;
    for (auto* formal = first; formal; formal = formal->nextArgument, ++count) {
        if (count > 3) return false;
        if (count) parameters += ",";
        if (formal->modifier == HLSLArgumentModifier_Out) parameters += "out ";
        parameters += std::string(GetTypeName(formal->type)) + " " + char('a' + count);
    }
    std::string body;
    if ((function == "tex2D" || function == "tex3D") && count == 4) body = "return textureGrad(a,b,c,d);";
    else if (function == "tex3Dgrad") body = "return textureGrad(a,b,c,d);";
    else if (function == "tex3Dbias") body = "return texture(a,b.xyz,b.w);";
    else if (function == "tex3Dproj") body = "return textureProj(a,b);";
    else if (function == "tex2Dsize" || function == "tex3Dsize") body = "return textureSize(a,0);";
    else if (function == "tex2Dfetch") body = "return texelFetch(a,b.xy,b.z);";
    else if (function == "all" || function == "any") {
        const bool all = function == "all";
        body = "return ";
        if (rows) {
            for (int c = 0; c < width; ++c) {
                if (c) body += all ? " && " : " || ";
                body += function + "(notEqual(a[" + std::to_string(c) + "],vec" + std::to_string(rows) + "(0)))";
            }
        } else if (width > 1) body += function + "(bvec" + std::to_string(width) + "(a))";
        else body += "bool(a)";
        body += ";";
    } else if (function == "frexp") {
        // ES 3.0 has bit reinterpretation, but does not require frexp. Normalize
        // subnormal inputs before splitting IEEE-754 exponent/mantissa bits.
        const auto scalar = NumericHelper("frexp:scalar", "float", "float x,out float e",
            "e=0.0; if(x==0.0 || isnan(x) || isinf(x)) return x; "
            "uint bits=floatBitsToUint(x); int power=int((bits>>23u)&255u); "
            "if(power==0){x*=16777216.0;bits=floatBitsToUint(x);power=int((bits>>23u)&255u)-24;} "
            "e=float(power-126);return uintBitsToFloat((bits&0x807fffffu)|0x3f000000u);");
        if (!rows && width == 1) body = "return " + scalar + "(a,b);";
        else {
            body = type + " result;";
            for (int c = 0; c < (rows ? width : 1); ++c) for (int r = 0; r < (rows ? rows : width); ++r) {
                const std::string index = (rows ? "[" + std::to_string(c) + "]" : "") + "[" + std::to_string(r) + "]";
                body += "result" + index + "=" + scalar + "(a" + index + ",b" + index + ");";
            }
            body += "return result;";
        }
    } else if (function == "modf" || function == "sincos") {
        if (function == "modf") {
            if (!rows) body = "return modf(a,b);";
            else {
                body = type + " result;";
                for (int c = 0; c < width; ++c) {
                    const std::string column = "[" + std::to_string(c) + "]";
                    body += "result" + column + "=modf(a" + column + ",b" + column + ");";
                }
                body += "return result;";
            }
        } else if (!rows) body = "b=sin(a);c=cos(a);";
        else for (int c = 0; c < width; ++c) {
            const std::string column = "[" + std::to_string(c) + "]";
            body += "b" + column + "=sin(a" + column + ");c" + column + "=cos(a" + column + ");";
        }
    } else if (function == "isfinite") {
        body = "return " + resultType + "(";
        for (int i = 0; i < width; ++i) {
            if (i) body += ",";
            const std::string x = "a" + (width > 1 ? "[" + std::to_string(i) + "]" : "");
            body += "!(isnan(" + x + ")||isinf(" + x + "))";
        }
        body += ");";
    } else if (function == "asfloat" || function == "asint" || function == "asuint") {
        const auto from = ScalarBaseType[base], to = ScalarBaseType[expression->expressionType.baseType];
        std::string cast = resultType;
        if (from == HLSLBaseType_Float && to == HLSLBaseType_Int) cast = "floatBitsToInt";
        if (from == HLSLBaseType_Float && to == HLSLBaseType_Uint) cast = "floatBitsToUint";
        if (from == HLSLBaseType_Int && to == HLSLBaseType_Float) cast = "intBitsToFloat";
        if (from == HLSLBaseType_Uint && to == HLSLBaseType_Float) cast = "uintBitsToFloat";
        body = "return " + cast + "(a);";
    } else if (function == "dot" && width == 1) body = "return a*b;";
    else if (function == "faceforward") body = "return -a*sign(" + std::string(width == 1 ? "b*c" : "dot(b,c)") + ");";
    else if (function == "lit") body = "return vec4(1.0,max(a,0.0),a<0.0?0.0:pow(max(b,0.0),c),1.0);";
    else if (function == "dst") body = "return vec4(1.0,a.y*b.y,a.z,b.w);";
    else {
        const bool custom = function == "rcp" || function == "mad" || function == "log10" || function == "ldexp" || function == "saturate";
        const bool columnWise = rows && function != "mul" && function != "transpose" && function != "determinant";
        if (!custom && !columnWise && function != "rsqrt" && function != "round") return false;
        std::string glsl = function;
        if (function == "rsqrt") glsl = "inversesqrt";
        else if (function == "round") glsl = "roundEven";
        else if (function == "frac") glsl = "fract";
        else if (function == "lerp") glsl = "mix";
        else if (function == "atan2") glsl = "atan";
        else if (function == "ddx") glsl = "dFdx";
        else if (function == "ddy") glsl = "dFdy";
        body = "return ";
        if (rows) body += type + "(";
        for (int c = 0; c < (rows ? width : 1); ++c) {
            if (c) body += ",";
            const std::string column = rows ? "[" + std::to_string(c) + "]" : "";
            const std::string a = "a" + column, b = "b" + column, d = "c" + column;
            if (function == "rcp") body += "1.0/" + a;
            else if (function == "mad") body += a + "*" + b + "+" + d;
            else if (function == "log10") body += "log(" + a + ")*0.4342944819032518";
            else if (function == "ldexp") body += a + "*exp2(" + b + ")";
            else if (function == "saturate") body += "clamp(" + a + ",0.0,1.0)";
            else if (function == "fmod") body += a + "-" + b + "*trunc(" + a + "/" + b + ")";
            else {
                body += glsl + "(" + a;
                if (count > 1) body += "," + b;
                if (count > 2) body += "," + d;
                body += ")";
            }
        }
        body += rows ? ");" : ";";
    }
    const auto helper = NumericHelper(function + ":" + resultType + parameters, resultType, parameters, body);
    m_writer.Write("%s(", helper.c_str());
    OutputExpressionList(expression->argument, expression->function->argument);
    m_writer.Write(")");
    return true;
}
