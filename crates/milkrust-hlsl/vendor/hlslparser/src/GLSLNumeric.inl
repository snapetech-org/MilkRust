// Independently authored HLSL-to-GLSL numeric lowering. Included in the M4
// namespace after GetTypeName, with the same compiler/output budgets.
// HLSL readonly locals/globals may require helper calls during lowering.
// Only retain GLSL const when the emitted expression is still a GLSL constant
// expression. Array extents are independently folded from the typed HLSL AST.
static bool DirectConstantExpression(HLSLExpression* expression);
static bool DirectConstantDeclaration(HLSLDeclaration* declaration) {
    DepthGuard depth;
    if (!declaration || !(declaration->type.flags & HLSLTypeFlag_Const) || !declaration->assignment) return false;
    for (auto* value = declaration->assignment; value; value = value->nextExpression)
        if (!DirectConstantExpression(value)) return false;
    return true;
}
static bool DirectConstantExpression(HLSLExpression* expression) {
    DepthGuard depth;
    if (!expression || !(expression->expressionType.flags & HLSLTypeFlag_Const) ||
        IsMatrixType(expression->expressionType.baseType) || expression->expressionType.baseType == HLSLBaseType_UserDefined) return false;
    switch (expression->nodeType) {
        case HLSLNodeType_LiteralExpression: return true;
        case HLSLNodeType_IdentifierExpression:
            return DirectConstantDeclaration(static_cast<HLSLIdentifierExpression*>(expression)->declaration);
        case HLSLNodeType_ConstructorExpression: {
            auto* constructor = static_cast<HLSLConstructorExpression*>(expression);
            if (constructor->type.array) return false;
            for (auto* a = constructor->argument; a; a = a->nextExpression)
                if (!DirectConstantExpression(a)) return false;
            return true;
        }
        case HLSLNodeType_CastingExpression:
            return DirectConstantExpression(static_cast<HLSLCastingExpression*>(expression)->expression);
        case HLSLNodeType_MemberAccess: {
            auto* member = static_cast<HLSLMemberAccess*>(expression);
            return !member->object->expressionType.array && IsVectorType(member->object->expressionType) &&
                DirectConstantExpression(member->object);
        }
        case HLSLNodeType_ArrayAccess: {
            auto* access = static_cast<HLSLArrayAccess*>(expression);
            return !access->array->expressionType.array && IsVectorType(access->array->expressionType) &&
                DirectConstantExpression(access->array) && DirectConstantExpression(access->index);
        }
        case HLSLNodeType_UnaryExpression: {
            auto* unary = static_cast<HLSLUnaryExpression*>(expression);
            if (unary->unaryOp != HLSLUnaryOp_Positive && unary->unaryOp != HLSLUnaryOp_Negative &&
                unary->unaryOp != HLSLUnaryOp_Not && unary->unaryOp != HLSLUnaryOp_BitNot) return false;
            return DirectConstantExpression(unary->expression);
        }
        case HLSLNodeType_BinaryExpression: {
            auto* binary = static_cast<HLSLBinaryExpression*>(expression);
            if (IsAssignOp(binary->binaryOp) ||
                (IsLogicOp(binary->binaryOp) && IsVectorType(binary->expressionType)) ||
                (binary->binaryOp == HLSLBinaryOp_Mod &&
                    (ScalarBaseType[binary->expressionType.baseType] == HLSLBaseType_Float ||
                     ScalarBaseType[binary->expressionType.baseType] == HLSLBaseType_Half))) return false;
            return DirectConstantExpression(binary->expression1) && DirectConstantExpression(binary->expression2);
        }
        case HLSLNodeType_ConditionalExpression: {
            auto* conditional = static_cast<HLSLConditionalExpression*>(expression);
            return IsScalarType(conditional->condition->expressionType) && DirectConstantExpression(conditional->condition) &&
                DirectConstantExpression(conditional->trueExpression) && DirectConstantExpression(conditional->falseExpression);
        }
        default: return false;
    }
}

struct MatrixSelection {
    HLSLExpression* matrix = NULL;
    HLSLExpression* row = NULL;
    HLSLExpression* index = NULL;
    int scalarSwizzleWidth = 1;
    std::vector<std::pair<int, int>> components; // HLSL row, column; -1 = row argument
};

static int SwizzleIndex(char c) {
    const char* alphabet = "xyzw";
    const char* p = strchr(alphabet, c);
    if (!p) { alphabet = "rgba"; p = strchr(alphabet, c); }
    return p ? int(p - alphabet) : -1;
}

static std::string MatrixComponent(const HLSLType& type, const std::string& value, int row, int column) {
    if (IsMatrixType(type.baseType)) {
        const int rows = MatrixRows(type.baseType), columns = NumericWidth(type.baseType);
        if (IsCustomMatrixType(type.baseType))
            return value + ".r" + std::to_string(row) + "[" + std::to_string(column) + "]";
        if (rows > 1 && columns > 1)
            return value + "[" + std::to_string(column) + "][" + std::to_string(row) + "]";
        if (rows == 1 && columns == 1) return value;
        return value + "[" + std::to_string(rows == 1 ? column : row) + "]";
    }
    if (IsScalarType(type)) return value;
    return value + "[" + std::to_string(column) + "]";
}

static bool SelectMatrix(HLSLExpression* expression, MatrixSelection& out) {
    DepthGuard depth;
    if (expression->nodeType == HLSLNodeType_MemberAccess) {
        auto* member = static_cast<HLSLMemberAccess*>(expression);
        if (!member->object->expressionType.array && IsMatrixType(member->object->expressionType.baseType)) {
            out.matrix = member->object;
            const char* p = member->field;
            while (*p) {
                if (*p++ != '_') throw std::invalid_argument("Invalid matrix selection");
                const int base = *p == 'm' ? (++p, '0') : '1';
                if (!p[0] || !p[1]) throw std::invalid_argument("Invalid matrix selection");
                const int r = p[0] - base, c = p[1] - base;
                if (r < 0 || r >= MatrixRows(out.matrix->expressionType.baseType) || c < 0 || c >= NumericWidth(out.matrix->expressionType.baseType))
                    throw std::invalid_argument("Matrix selection is out of bounds");
                out.components.push_back({r, c}); p += 2;
            }
            return true;
        }
        if (SelectMatrix(member->object, out)) {
            if (out.index) {
                // A dynamic component is a scalar, which HLSL may replicate
                // with .xx/.rrrr. Keep the index expression evaluated once.
                for (const char* p = member->field; *p; ++p)
                    if (SwizzleIndex(*p) < 0 || SwizzleIndex(*p) >= out.scalarSwizzleWidth)
                        throw std::invalid_argument("Invalid indexed matrix scalar swizzle");
                out.scalarSwizzleWidth = int(strlen(member->field));
                return true;
            }
            const auto original = out.components;
            out.components.clear();
            for (const char* p = member->field; *p; ++p) {
                int index = SwizzleIndex(*p);
                if (index < 0 || size_t(index) >= original.size()) throw std::invalid_argument("Invalid matrix row swizzle");
                out.components.push_back(original[index]);
            }
            return true;
        }
    } else if (expression->nodeType == HLSLNodeType_ArrayAccess) {
        auto* access = static_cast<HLSLArrayAccess*>(expression);
        if (!access->array->expressionType.array && IsMatrixType(access->array->expressionType.baseType)) {
            out.matrix = access->array; out.row = access->index;
            for (int c = 0; c < NumericWidth(out.matrix->expressionType.baseType); ++c) out.components.push_back({-1, c});
            return true;
        }
        if (SelectMatrix(access->array, out)) {
            if (out.index) throw std::invalid_argument("Invalid indexed matrix selection");
            out.index = access->index;
            return true;
        }
    }
    return false;
}

std::string GLSLGenerator::NumericHelper(const std::string& key, const std::string& type,
    const std::string& parameters, const std::string& body) {
    const auto found = m_numericHelperNames.find(key);
    if (found != m_numericHelperNames.end()) return found->second;
    if (m_numericHelperNames.size() >= 512) throw std::length_error("HLSL numeric helper limit exceeded");
    char name[64];
    const std::string base = "milkrust_numeric_" + std::to_string(m_numericHelperNames.size()) + "_";
    ChooseUniqueName(base.c_str(), name, sizeof(name));
    m_numericHelpers.WriteLine(0, "%s %s(%s) { %s }", type.c_str(), name, parameters.c_str(), body.c_str());
    m_numericHelperNames[key] = name;
    return name;
}

bool GLSLGenerator::OutputMatrixSelection(HLSLExpression* expression, HLSLExpression* value,
    const char* operation, bool postfix) {
    MatrixSelection selection;
    if (!SelectMatrix(expression, selection)) return false;
    const int count = int(selection.components.size());
    if (count < 1 || count > 4) throw std::invalid_argument("Invalid matrix selection width");
    const bool write = operation != NULL;
    if (write && selection.index && selection.scalarSwizzleWidth > 1)
        throw std::invalid_argument("Matrix assignment repeats a destination component");
    const HLSLType& matrixHlslType = selection.matrix->expressionType;
    const int rows = MatrixRows(matrixHlslType.baseType), columns = NumericWidth(matrixHlslType.baseType);
    const bool custom = IsCustomMatrixType(matrixHlslType.baseType);
    const std::string matrixType = GetTypeName(matrixHlslType);
    const std::string type = GetTypeName(expression->expressionType);
    std::string parameters = (write ? "inout " : "") + matrixType + " m";
    if (selection.row) parameters += ", int i";
    if (selection.index) parameters += ", int j";
    if (value) parameters += ", " + type + " v";
    std::vector<std::string> components;
    std::vector<std::string> targets;
    const std::string rowVectorType = selection.row && selection.index
        ? GetTypeName(HLSLType(NumericShape(ScalarBaseType[matrixHlslType.baseType], 0, columns))) : type;
    std::string read = count == 1 ? "" : rowVectorType + "(";
    for (int n = 0; n < count; ++n) {
        const auto item = selection.components[n];
        std::string component;
        std::string target;
        if (item.first >= 0) {
            component = MatrixComponent(matrixHlslType, "m", item.first, item.second);
            target = component;
        } else if (!selection.row || rows == 1) {
            component = MatrixComponent(matrixHlslType, "m", 0, item.second);
            target = rows == 1 && !selection.index ? "m[" + std::to_string(item.second) + "]" : MatrixComponent(matrixHlslType, "m", 0, item.second);
        } else if (custom) {
            component = "(";
            for (int r = rows - 1; r >= 0; --r) {
                if (r != rows - 1) component += ":";
                component += "i==" + std::to_string(r) + "?m.r" + std::to_string(r) + "[" + std::to_string(item.second) + "]";
            }
            component += ":" + GetTypeName(HLSLType(ScalarBaseType[matrixHlslType.baseType])) + "(0))";
            target = "dynamic-row:" + std::to_string(item.second);
        } else if (columns == 1) {
            component = "m[i]";
            target = "m[i]";
        } else {
            component = "m[" + std::to_string(item.second) + "][i]";
            target = component;
        }
        if (write && !selection.index && std::find(targets.begin(), targets.end(), target) != targets.end())
            throw std::invalid_argument("Matrix assignment repeats a destination component");
        components.push_back(component); targets.push_back(target);
        if (n) read += ",";
        read += component;
    }
    if (count > 1) read += ")";
    if (selection.index && !(rows > 1 && columns == 1)) {
        read = "(" + read + ")[j]";
        if (selection.scalarSwizzleWidth > 1) read = type + "(" + read + ")";
    }
    std::string body;
    if (selection.row) body += "if (i < 0 || i >= " + std::to_string(rows) + ") return " + type + "(0); ";
    if (selection.index) body += "if (j < 0 || j >= " + std::to_string(count) + ") return " + type + "(0); ";
    if (!write) body += "return " + read + ";";
    else {
        if (strcmp(operation, "=") != 0) body += type + " oldValue = " + read + "; ";
        body += type + " nextValue = ";
        if (strcmp(operation, "=") == 0) body += "v; ";
        else if (strcmp(operation, "%") == 0) body += "oldValue - v * trunc(oldValue / v); ";
        else body += "oldValue " + std::string(operation) + " " + (value ? "v" : type + "(1)") + "; ";
        for (int n = 0; n < count; ++n) {
            const std::string rhs = (selection.index || count == 1) ? "nextValue" : "nextValue[" + std::to_string(n) + "]";
            if (selection.index) body += "if (j == " + std::to_string(n) + ") ";
            if (selection.row && rows == 1) {
                body += "m[" + std::to_string(selection.components[n].second) + "] = " + rhs + "; ";
            } else if (selection.row && custom) {
                body += "if (i == 0) m.r0[" + std::to_string(selection.components[n].second) + "] = " + rhs + "; ";
                for (int r = 1; r < rows; ++r)
                    body += "else if (i == " + std::to_string(r) + ") m.r" + std::to_string(r) + "[" + std::to_string(selection.components[n].second) + "] = " + rhs + "; ";
            } else if (selection.row && columns == 1) {
                body += "m[i] = " + rhs + "; ";
            } else if (selection.row) {
                body += "m[" + std::to_string(selection.components[n].second) + "][i] = " + rhs + "; ";
            } else {
                body += components[n] + " = " + rhs + "; ";
            }
        }
        body += std::string("return ") + (postfix ? "oldValue;" : "nextValue;");
    }
    const std::string name = NumericHelper(parameters + body, type, parameters, body);
    m_writer.Write("%s(", name.c_str());
    OutputExpression(selection.matrix);
    if (selection.row) { m_writer.Write(", "); OutputExpression(selection.row, &kIntType); }
    if (selection.index) { m_writer.Write(", "); OutputExpression(selection.index, &kIntType); }
    if (value) { m_writer.Write(", "); OutputExpression(value, &expression->expressionType); }
    m_writer.Write(")");
    return true;
}

HLSLIdentifierExpression* GLSLGenerator::LocalTemporary(const HLSLType& type) {
    if (!m_insideFunction || ++m_temporaryCount > 4096) throw std::length_error("HLSL temporary limit exceeded");
    char name[64];
    const std::string base = "milkrust_temporary_" + std::to_string(m_temporaryCount) + "_";
    ChooseUniqueName(base.c_str(), name, sizeof(name));
    auto* declaration = m_tree->AddNode<HLSLDeclaration>(NULL, 0);
    declaration->name = m_tree->AddString(name);
    declaration->type = type; declaration->type.flags = 0;
    m_localTemporaries.push_back(declaration);
    auto* identifier = m_tree->AddNode<HLSLIdentifierExpression>(NULL, 0);
    identifier->name = declaration->name; identifier->expressionType = declaration->type;
    return identifier;
}

void GLSLGenerator::InsertLocalTemporaries(size_t offset) {
    CodeWriter declarations(false);
    std::swap(m_writer, declarations);
    for (auto* declaration : m_localTemporaries) {
        m_writer.BeginLine(1); OutputDeclaration(declaration); m_writer.EndLine(";");
    }
    std::swap(m_writer, declarations);
    m_writer.Insert(offset, declarations.GetResult());
    m_localTemporaries.clear();
    m_insideFunction = false;
}

HLSLExpression* GLSLGenerator::FreezeLvalue(HLSLExpression* expression) {
    DepthGuard depth;
    if (expression->expressionType.flags & HLSLTypeFlag_Const) throw std::invalid_argument("Cannot write a const HLSL argument");
    if (expression->nodeType == HLSLNodeType_IdentifierExpression) return expression;
    if (expression->nodeType == HLSLNodeType_MemberAccess) {
        auto* member = static_cast<HLSLMemberAccess*>(expression);
        auto* frozen = m_tree->AddNode<HLSLMemberAccess>(expression->fileName, expression->line);
        *frozen = *member;
        frozen->object = FreezeLvalue(member->object);
        return frozen;
    }
    if (expression->nodeType == HLSLNodeType_ArrayAccess) {
        auto* access = static_cast<HLSLArrayAccess*>(expression);
        auto* frozen = m_tree->AddNode<HLSLArrayAccess>(expression->fileName, expression->line);
        *frozen = *access;
        frozen->array = FreezeLvalue(access->array);
        auto* index = LocalTemporary(kIntType);
        m_writer.Write("%s = ", index->name); OutputExpression(access->index, &kIntType); m_writer.Write(", ");
        frozen->index = index;
        return frozen;
    }
    throw std::invalid_argument("HLSL output argument requires a writable variable or selection");
}

bool GLSLGenerator::OutputCallWithCopyOut(HLSLFunctionCall* expression) {
    bool needed = false;
    auto* formal = expression->function->argument;
    for (auto* actual = expression->argument; actual && formal; actual = actual->nextExpression, formal = formal->nextArgument) {
        if (formal->modifier == HLSLArgumentModifier_Out || formal->modifier == HLSLArgumentModifier_Inout) {
            MatrixSelection selection;
            needed |= SelectMatrix(actual, selection) || actual->expressionType.baseType != formal->type.baseType;
        }
    }
    if (!needed) return false;
    std::vector<std::pair<HLSLExpression*, HLSLIdentifierExpression*>> outputs;
    HLSLExpression* first = NULL;
    HLSLExpression* last = NULL;
    formal = expression->function->argument;
    m_writer.Write("(");
    for (auto* actual = expression->argument; actual && formal; actual = actual->nextExpression, formal = formal->nextArgument) {
        HLSLExpression* argument;
        if (IsSamplerType(formal->type.baseType)) {
            if (actual->nodeType != HLSLNodeType_IdentifierExpression) throw std::invalid_argument("Sampler argument must name a binding");
            auto* clone = m_tree->AddNode<HLSLIdentifierExpression>(NULL, 0);
            *clone = *static_cast<HLSLIdentifierExpression*>(actual); argument = clone;
        } else {
            auto* temporary = LocalTemporary(formal->type);
            const bool output = formal->modifier == HLSLArgumentModifier_Out || formal->modifier == HLSLArgumentModifier_Inout;
            auto* source = output ? FreezeLvalue(actual) : actual;
            if (formal->modifier != HLSLArgumentModifier_Out) {
                m_writer.Write("%s = ", temporary->name); OutputExpression(source, &formal->type); m_writer.Write(", ");
            }
            if (output) outputs.push_back({source, temporary});
            argument = temporary;
        }
        argument->nextExpression = NULL;
        if (last) last->nextExpression = argument; else first = argument;
        last = argument;
    }
    HLSLFunctionCall call = *expression; call.argument = first;
    HLSLIdentifierExpression* result = NULL;
    if (expression->expressionType.baseType != HLSLBaseType_Void) {
        result = LocalTemporary(expression->expressionType); m_writer.Write("%s = ", result->name);
    }
    OutputExpression(&call);
    for (const auto& output : outputs) {
        m_writer.Write(", ");
        if (!OutputMatrixSelection(output.first, output.second, "=")) {
            OutputExpression(output.first); m_writer.Write(" = "); OutputExpression(output.second, &output.first->expressionType);
        }
    }
    m_writer.Write(", %s)", result ? result->name : "0");
    return true;
}

void GLSLGenerator::OutputConversion(HLSLExpression* expression, const HLSLType& type) {
    const HLSLType sourceType = expression->expressionType;
    const auto source = sourceType.baseType;
    const auto target = type.baseType;
    if (sourceType.array || type.array || source == HLSLBaseType_UserDefined || target == HLSLBaseType_UserDefined) {
        OutputAggregateConversion(expression, type); return;
    }
    if (IsMatrixType(target) && IsScalarType(source)) {
        const std::string targetName = GetTypeName(type);
        const auto scalar = ScalarBaseType[target];
        const int rows = MatrixRows(target), columns = NumericWidth(target);
        std::string body = "return " + targetName + "(";
        if (IsCustomMatrixType(target)) {
            const std::string rowType = GetTypeName(HLSLType(NumericShape(scalar, 0, columns)));
            for (int r = 0; r < rows; ++r) {
                if (r) body += ",";
                body += rowType + "(";
                for (int c = 0; c < columns; ++c) {
                    if (c) body += ",";
                    body += GetTypeName(HLSLType(scalar)) + "(s)";
                }
                body += ")";
            }
        } else {
            for (int i = 0; i < rows * columns; ++i) {
                if (i) body += ",";
                body += GetTypeName(HLSLType(scalar)) + "(s)";
            }
        }
        body += ");";
        const std::string scalarName = GetTypeName(HLSLType(source));
        const std::string helper = NumericHelper("splat:" + targetName + scalarName,
            targetName, scalarName + " s", body);
        m_writer.Write("%s(", helper.c_str()); OutputExpression(expression); m_writer.Write(")");
        return;
    }
    if (IsMatrixType(source)) {
        const int sourceRows = MatrixRows(source), sourceColumns = NumericWidth(source);
        const int sourceCount = sourceRows * sourceColumns;
        const std::string sourceName = GetTypeName(sourceType);
        const std::string targetName = GetTypeName(type);
        const bool targetMatrix = IsMatrixType(target);
        const bool targetVector = IsVectorType(target);
        const bool targetScalar = IsScalarType(target);
        if (!targetMatrix && !targetVector && !targetScalar)
            throw std::invalid_argument("Unsupported HLSL matrix conversion target");
        const int targetRows = targetMatrix ? MatrixRows(target) : 1;
        const int targetColumns = targetMatrix ? NumericWidth(target) : targetVector ? NumericWidth(target) : 1;
        if (targetRows * targetColumns > sourceCount)
            throw std::invalid_argument("HLSL matrix conversion cannot expand its source");
        const HLSLBaseType targetScalarType = ScalarBaseType[target];
        std::string body;
        if (IsCustomMatrixType(target)) {
            const std::string rowType = GetTypeName(HLSLType(NumericShape(targetScalarType, 0, targetColumns)));
            body = "return " + targetName + "(";
            for (int r = 0; r < targetRows; ++r) {
                if (r) body += ",";
                body += rowType + "(";
                for (int c = 0; c < targetColumns; ++c) {
                    if (c) body += ",";
                    const int linear = r * targetColumns + c;
                    const std::string value = MatrixComponent(sourceType, "m", linear / sourceColumns, linear % sourceColumns);
                    body += GetTypeName(HLSLType(targetScalarType)) + "(" + value + ")";
                }
                body += ")";
            }
            body += ");";
        } else if (targetMatrix && targetRows > 1 && targetColumns > 1) {
            body = "return " + targetName + "(";
            for (int c = 0; c < targetColumns; ++c) for (int r = 0; r < targetRows; ++r) {
                if (c || r) body += ",";
                const int linear = r * targetColumns + c;
                const std::string value = MatrixComponent(sourceType, "m", linear / sourceColumns, linear % sourceColumns);
                body += GetTypeName(HLSLType(targetScalarType)) + "(" + value + ")";
            }
            body += ");";
        } else if (targetMatrix) {
            body = "return " + targetName + "(";
            for (int i = 0; i < targetRows * targetColumns; ++i) {
                if (i) body += ",";
                const std::string value = MatrixComponent(sourceType, "m", i / sourceColumns, i % sourceColumns);
                body += GetTypeName(HLSLType(targetScalarType)) + "(" + value + ")";
            }
            body += ");";
        } else if (targetScalar) {
            body = "return " + targetName + "(" + GetTypeName(HLSLType(targetScalarType)) + "(" +
                MatrixComponent(sourceType, "m", 0, 0) + "));";
        } else {
            body = "return " + targetName + "(";
            for (int c = 0; c < targetColumns; ++c) {
                if (c) body += ",";
                const int linear = c;
                const std::string value = MatrixComponent(sourceType, "m", linear / sourceColumns, linear % sourceColumns);
                body += GetTypeName(HLSLType(targetScalarType)) + "(" + value + ")";
            }
            body += ");";
        }
        const std::string helper = NumericHelper("matrix-convert:" + sourceName + ":" + targetName + body,
            targetName, sourceName + " m", body);
        m_writer.Write("%s(", helper.c_str()); OutputExpression(expression); m_writer.Write(")");
        return;
    }
    m_writer.Write("%s(", GetTypeName(type));
    OutputExpression(expression);
    m_writer.Write(")");
}

void GLSLGenerator::OutputMatrixConstructor(HLSLConstructorExpression* expression) {
    HLSLExpression* argument = expression->argument;
    if (argument && !argument->nextExpression && (IsScalarType(argument->expressionType) || IsMatrixType(argument->expressionType.baseType))) {
        OutputConversion(argument, expression->type); return;
    }
    const auto matrix = expression->type.baseType;
    std::string parameters;
    std::vector<std::string> values;
    int n = 0;
    for (auto* a = argument; a; a = a->nextExpression, ++n) {
        if (n > 16) throw std::invalid_argument("Too many matrix constructor arguments");
        const std::string name = "a" + std::to_string(n);
        if (n) parameters += ",";
        parameters += std::string(GetTypeName(a->expressionType)) + " " + name;
        const int rows = MatrixRows(a->expressionType.baseType), width = NumericWidth(a->expressionType.baseType);
        if (!width || a->expressionType.array) throw std::invalid_argument("Matrix constructor requires numeric arguments");
        if (rows) for (int r = 0; r < rows; ++r) for (int c = 0; c < width; ++c) values.push_back(MatrixComponent(a->expressionType, name, r, c));
        else for (int c = 0; c < width; ++c) values.push_back(name + (width == 1 ? "" : "[" + std::to_string(c) + "]"));
    }
    const int rows = MatrixRows(matrix), width = NumericWidth(matrix);
    if (int(values.size()) != rows * width) throw std::invalid_argument("Incorrect matrix constructor component count");
    const std::string type = GetTypeName(expression->type);
    const HLSLBaseType scalar = ScalarBaseType[matrix];
    std::string body = "return " + type + "(";
    if (IsCustomMatrixType(matrix)) {
        const std::string rowType = GetTypeName(HLSLType(NumericShape(scalar, 0, width)));
        for (int r = 0; r < rows; ++r) {
            if (r) body += ",";
            body += rowType + "(";
            for (int c = 0; c < width; ++c) { if (c) body += ","; body += GetTypeName(HLSLType(scalar)) + "(" + values[r * width + c] + ")"; }
            body += ")";
        }
    } else {
        for (int c = 0; c < width; ++c) for (int r = 0; r < rows; ++r) {
            if (c || r) body += ",";
            body += GetTypeName(HLSLType(scalar)) + "(" + values[r * width + c] + ")";
        }
    }
    body += ");";
    const auto name = NumericHelper("ctor:" + type + parameters, type, parameters, body);
    m_writer.Write("%s(", name.c_str()); OutputExpressionList(argument); m_writer.Write(")");
}
