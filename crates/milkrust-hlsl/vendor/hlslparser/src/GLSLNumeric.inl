// Independently authored HLSL-to-GLSL numeric lowering. Included in the M4
// namespace after GetTypeName, with the same compiler/output budgets.
struct MatrixSelection {
    HLSLExpression* matrix = NULL;
    HLSLExpression* row = NULL;
    HLSLExpression* index = NULL;
    std::vector<std::pair<int, int>> components; // HLSL row, column; -1 = row argument
};

static int SwizzleIndex(char c) {
    const char* alphabet = "xyzw";
    const char* p = strchr(alphabet, c);
    if (!p) { alphabet = "rgba"; p = strchr(alphabet, c); }
    return p ? int(p - alphabet) : -1;
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
            if (out.index) throw std::invalid_argument("Swizzling an indexed matrix selection is unsupported");
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
    const std::string matrixType = GetTypeName(selection.matrix->expressionType);
    const std::string type = GetTypeName(expression->expressionType);
    std::string parameters = (write ? "inout " : "") + matrixType + " m";
    if (selection.row) parameters += ", int i";
    if (selection.index) parameters += ", int j";
    if (value) parameters += ", " + type + " v";
    std::vector<std::string> components;
    std::string read = count == 1 ? "" : "vec" + std::to_string(count) + "(";
    for (int n = 0; n < count; ++n) {
        const auto item = selection.components[n];
        const std::string component = "m[" + std::to_string(item.second) + "][" + (item.first < 0 ? "i" : std::to_string(item.first)) + "]";
        if (write && !selection.index && std::find(components.begin(), components.end(), component) != components.end())
            throw std::invalid_argument("Matrix assignment repeats a destination component");
        components.push_back(component);
        if (n) read += ",";
        read += component;
    }
    if (count > 1) read += ")";
    if (selection.index) read = "(" + read + ")[j]";
    std::string body;
    if (selection.row) body += "if (i < 0 || i >= " + std::to_string(MatrixRows(selection.matrix->expressionType.baseType)) + ") return " + type + "(0); ";
    if (selection.index) body += "if (j < 0 || j >= " + std::to_string(count) + ") return " + type + "(0); ";
    if (!write) body += "return " + read + ";";
    else {
        if (strcmp(operation, "=") != 0) body += type + " oldValue = " + read + "; ";
        body += type + " nextValue = ";
        if (strcmp(operation, "=") == 0) body += "v; ";
        else if (strcmp(operation, "%") == 0) body += "oldValue - v * trunc(oldValue / v); ";
        else body += "oldValue " + std::string(operation) + " " + (value ? "v" : type + "(1)") + "; ";
        for (int n = 0; n < count; ++n) {
            if (selection.index) body += "if (j == " + std::to_string(n) + ") ";
            body += components[n] + " = nextValue" + (selection.index || count == 1 ? "" : "[" + std::to_string(n) + "]") + "; ";
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
    const auto source = expression->expressionType.baseType;
    const auto target = type.baseType;
    if (IsMatrixType(target) && IsScalarType(source)) {
        const std::string matrix = GetTypeName(type);
        std::string body = "return " + matrix + "(";
        for (int c = 0; c < NumericWidth(target); ++c) {
            if (c) body += ",";
            body += "vec" + std::to_string(MatrixRows(target)) + "(s)";
        }
        body += ");";
        const auto name = NumericHelper("splat:" + matrix, matrix, "float s", body);
        m_writer.Write("%s(", name.c_str()); OutputExpression(expression, &kFloatType); m_writer.Write(")");
        return;
    }
    m_writer.Write("%s(", GetTypeName(type));
    if (IsScalarType(target) && (IsVectorType(source) || IsMatrixType(source))) {
        m_writer.Write("("); OutputExpression(expression); m_writer.Write(IsMatrixType(source) ? ")[0][0]" : ").x");
    } else if (IsVectorType(target) && IsMatrixType(source)) {
        const std::string matrix = GetTypeName(expression->expressionType);
        const int count = NumericWidth(target);
        if (count > NumericWidth(source)) throw std::invalid_argument("Invalid matrix to vector truncation");
        std::string body = "return " + std::string(GetTypeName(type)) + "(";
        for (int c = 0; c < count; ++c) { if (c) body += ","; body += "m[" + std::to_string(c) + "][0]"; }
        body += ");";
        const auto name = NumericHelper("rowcast:" + matrix + GetTypeName(type), GetTypeName(type), matrix + " m", body);
        m_writer.Write("%s(", name.c_str()); OutputExpression(expression); m_writer.Write(")");
    } else OutputExpression(expression);
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
        if (rows) for (int r = 0; r < rows; ++r) for (int c = 0; c < width; ++c) values.push_back(name + "[" + std::to_string(c) + "][" + std::to_string(r) + "]");
        else for (int c = 0; c < width; ++c) values.push_back(name + (width == 1 ? "" : "[" + std::to_string(c) + "]"));
    }
    const int rows = MatrixRows(matrix), width = NumericWidth(matrix);
    if (int(values.size()) != rows * width) throw std::invalid_argument("Incorrect matrix constructor component count");
    const std::string type = GetTypeName(expression->type);
    std::string body = "return " + type + "(";
    for (int c = 0; c < width; ++c) for (int r = 0; r < rows; ++r) {
        if (c || r) body += ",";
        body += "float(" + values[r * width + c] + ")";
    }
    body += ");";
    const auto name = NumericHelper("ctor:" + type + parameters, type, parameters, body);
    m_writer.Write("%s(", name.c_str()); OutputExpressionList(argument); m_writer.Write(")");
}
