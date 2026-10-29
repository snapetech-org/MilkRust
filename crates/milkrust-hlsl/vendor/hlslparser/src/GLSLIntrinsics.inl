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
    if ((function == "tex2Doffset" || function == "tex3Doffset") && expression->function->numArguments == 3) {
        auto* sampler = expression->argument;
        auto* coordinate = sampler->nextExpression;
        auto* offset = coordinate->nextExpression;
        m_writer.Write("textureOffset("); OutputExpression(sampler); m_writer.Write(",");
        OutputExpression(coordinate); m_writer.Write(",");
        OutputExpression(offset, &first->nextArgument->nextArgument->type);
        m_writer.Write(")"); return true;
    }
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
    if (function == "tex3Dlod" && expression->function->numArguments == 3) {
        auto* sampler = expression->argument;
        auto* coordinate = sampler->nextExpression;
        auto* offset = coordinate->nextExpression;
        auto* temporary = LocalTemporary(first->nextArgument->type);
        m_writer.Write("(%s = ", temporary->name);
        OutputExpression(coordinate, &temporary->expressionType);
        m_writer.Write(", textureLodOffset("); OutputExpression(sampler);
        m_writer.Write(", %s.xyz, %s.w, ", temporary->name, temporary->name);
        // GLSL requires the offset to remain a constant expression.
        OutputExpression(offset, &first->nextArgument->nextArgument->type);
        m_writer.Write("))"); return true;
    }
    if ((function == "tex2Dbias" || function == "tex3Dbias") && expression->function->numArguments == 3) {
        auto* sampler = expression->argument;
        auto* coordinate = sampler->nextExpression;
        auto* offset = coordinate->nextExpression;
        auto* temporary = LocalTemporary(first->nextArgument->type);
        m_writer.Write("(%s = ", temporary->name);
        OutputExpression(coordinate, &temporary->expressionType);
        m_writer.Write(", textureOffset("); OutputExpression(sampler);
        if (function == "tex2Dbias") m_writer.Write(", %s.xy, ", temporary->name);
        else m_writer.Write(", %s.xyz, ", temporary->name);
        OutputExpression(offset, &first->nextArgument->nextArgument->type);
        m_writer.Write(", %s.w))", temporary->name); return true;
    }
    if ((function == "tex2Dgrad" || function == "tex3Dgrad") && expression->function->numArguments == 5) {
        m_writer.Write("textureGradOffset(");
        auto* argument = expression->argument;
        for (int index = 0; argument && index < 5; ++index, argument = argument->nextExpression) {
            if (index) m_writer.Write(",");
            if (index == 4) OutputExpression(argument, &first->nextArgument->nextArgument->nextArgument->nextArgument->type);
            else OutputExpression(argument);
        }
        m_writer.Write(")"); return true;
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
    else if (function == "tex3Dfetch") body = "return texelFetch(a,b.xyz,b.w);";
    else if (function == "mul" && count == 2) {
        const HLSLType& left = first->type;
        const HLSLType& right = first->nextArgument->type;
        const bool leftMatrix = IsMatrixType(left.baseType), rightMatrix = IsMatrixType(right.baseType);
        const bool leftVector = IsVectorType(left.baseType), rightVector = IsVectorType(right.baseType);
        const bool leftScalar = IsScalarType(left), rightScalar = IsScalarType(right);
        if (!(leftMatrix || leftVector || leftScalar) || !(rightMatrix || rightVector || rightScalar))
            return false;
        auto rows = [](const HLSLType& value) { return IsMatrixType(value.baseType) ? MatrixRows(value.baseType) : 1; };
        auto columns = [](const HLSLType& value) { return IsMatrixType(value.baseType) || IsVectorType(value.baseType) ? NumericWidth(value.baseType) : 1; };
        auto component = [](const HLSLType& value, const std::string& name, int row, int column, bool rightVectorValue) {
            if (IsScalarType(value)) return name;
            if (IsMatrixType(value.baseType)) return MatrixComponent(value, name, row, column);
            return name + "[" + std::to_string(rightVectorValue ? row : column) + "]";
        };
        if (leftVector && rightVector) {
            body = "return (";
            for (int i = 0; i < NumericWidth(left.baseType); ++i) {
                if (i) body += "+";
                body += "a[" + std::to_string(i) + "]*b[" + std::to_string(i) + "]";
            }
            body += ");";
        } else if (leftScalar || rightScalar) {
            const HLSLType& shaped = leftScalar ? right : left;
            const int outRows = rows(shaped), outColumns = columns(shaped);
            const bool customResult = IsCustomMatrixType(expression->expressionType.baseType);
            if (customResult) body = resultType + " result;";
            else if (IsMatrixType(expression->expressionType.baseType) && outRows > 1 && outColumns > 1)
                body = "return " + resultType + "(";
            else body = "return " + resultType + "(";
            auto valueAt = [&](int r, int c) {
                return leftScalar ? "a*" + component(right, "b", r, c, false)
                    : component(left, "a", r, c, false) + "*b";
            };
            if (customResult) {
                for (int r = 0; r < outRows; ++r) {
                    if (r) body += ";";
                    body += "result.r" + std::to_string(r) + "=" + GetTypeName(HLSLType(NumericShape(ScalarBaseType[expression->expressionType.baseType], 0, outColumns))) + "(";
                    for (int c = 0; c < outColumns; ++c) { if (c) body += ","; body += valueAt(r, c); }
                    body += ")";
                }
                body += ";return result;";
            } else if (IsMatrixType(expression->expressionType.baseType) && outRows > 1 && outColumns > 1) {
                for (int c = 0; c < outColumns; ++c) for (int r = 0; r < outRows; ++r) {
                    if (c || r) body += ",";
                    body += valueAt(r, c);
                }
                body += ");";
            } else {
                const int outCount = outRows * outColumns;
                for (int i = 0; i < outCount; ++i) {
                    if (i) body += ",";
                    const int r = outRows == 1 ? 0 : i;
                    const int c = outRows == 1 ? i : 0;
                    body += valueAt(r, c);
                }
                body += ");";
            }
        } else {
            const int leftRows = leftMatrix ? rows(left) : 1;
            const int leftColumns = columns(left);
            const int rightColumns = rightMatrix ? columns(right) : 1;
            const bool customResult = IsCustomMatrixType(expression->expressionType.baseType);
            const int outRows = leftRows, outColumns = rightColumns;
            const std::string zero = GetTypeName(HLSLType(ScalarBaseType[expression->expressionType.baseType])) + "(0)";
            auto sum = [&](int r, int c) {
                std::string value = "(";
                for (int k = 0; k < leftColumns; ++k) {
                    if (k) value += "+";
                    value += component(left, "a", r, k, false) + "*" + component(right, "b", k, c, true);
                }
                if (leftColumns == 0) value += zero;
                return value + ")";
            };
            if (customResult) body = resultType + " result;";
            else body = "return " + resultType + "(";
            if (customResult) {
                for (int r = 0; r < outRows; ++r) {
                    body += "result.r" + std::to_string(r) + "=" + GetTypeName(HLSLType(NumericShape(ScalarBaseType[expression->expressionType.baseType], 0, outColumns))) + "(";
                    for (int c = 0; c < outColumns; ++c) { if (c) body += ","; body += sum(r, c); }
                    body += ");";
                }
                body += "return result;";
            } else if (IsMatrixType(expression->expressionType.baseType) && outRows > 1 && outColumns > 1) {
                for (int c = 0; c < outColumns; ++c) for (int r = 0; r < outRows; ++r) {
                    if (c || r) body += ",";
                    body += sum(r, c);
                }
                body += ");";
            } else {
                for (int i = 0; i < outRows * outColumns; ++i) {
                    if (i) body += ",";
                    body += sum(outRows == 1 ? 0 : i, outRows == 1 ? i : 0);
                }
                body += ");";
            }
        }
    } else if (function == "transpose" && count == 1 && IsMatrixType(base)) {
        const int sourceRows = MatrixRows(base), sourceColumns = NumericWidth(base);
        const int resultRows = sourceColumns, resultColumns = sourceRows;
        const bool customResult = IsCustomMatrixType(expression->expressionType.baseType);
        if (customResult) body = resultType + " result;";
        else if (resultRows > 1 && resultColumns > 1) body = "return " + resultType + "(";
        else if (resultRows * resultColumns == 1) body = "return a;";
        else body = "return " + resultType + "(";
        if (customResult) {
            for (int r = 0; r < resultRows; ++r) {
                body += "result.r" + std::to_string(r) + "=" + GetTypeName(HLSLType(NumericShape(ScalarBaseType[expression->expressionType.baseType], 0, resultColumns))) + "(";
                for (int c = 0; c < resultColumns; ++c) { if (c) body += ","; body += MatrixComponent(first->type, "a", c, r); }
                body += ");";
            }
            body += "return result;";
        } else if (resultRows > 1 && resultColumns > 1) {
            for (int c = 0; c < resultColumns; ++c) for (int r = 0; r < resultRows; ++r) {
                if (c || r) body += ",";
                body += MatrixComponent(first->type, "a", c, r);
            }
            body += ");";
        } else if (resultRows * resultColumns != 1) {
            for (int i = 0; i < resultRows * resultColumns; ++i) {
                if (i) body += ",";
                body += MatrixComponent(first->type, "a", resultRows == 1 ? i : 0, resultRows == 1 ? 0 : i);
            }
            body += ");";
        }
    } else if (function == "all" || function == "any") {
        const bool all = function == "all";
        body = "return ";
        if (rows) {
            bool firstComponent = true;
            for (int r = 0; r < rows; ++r) for (int c = 0; c < width; ++c) {
                if (!firstComponent) body += all ? " && " : " || ";
                firstComponent = false;
                body += "bool(" + MatrixComponent(first->type, "a", r, c) + ")";
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
