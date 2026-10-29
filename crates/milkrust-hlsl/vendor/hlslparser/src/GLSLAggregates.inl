// Independently authored legacy aggregate initialization/casts. Included after
// numeric lowering. Expanded values reference temporaries, so a function call,
// array index or increment in an initializer is never duplicated per component.
static void AggregateValues(HLSLTree& tree, const HLSLType& type,
    const std::string& value, std::vector<std::string>& out) {
    DepthGuard depth;
    if (type.array) {
        const int count = AggregateArrayExtent(tree, type);
        HLSLType element = type; element.array = false; element.arraySize = NULL;
        for (int i = 0; i < count; ++i)
            AggregateValues(tree, element, value + "[" + std::to_string(i) + "]", out);
    } else if (type.baseType == HLSLBaseType_UserDefined) {
        auto* structure = tree.FindGlobalStruct(type.typeName);
        if (!structure || !structure->field) throw std::invalid_argument("Invalid HLSL aggregate structure");
        for (auto* field = structure->field; field; field = field->nextField)
            AggregateValues(tree, field->type, value + "." + field->name, out);
    } else {
        const int width = NumericWidth(type.baseType), rows = MatrixRows(type.baseType);
        if (!width) throw std::invalid_argument("HLSL aggregate requires numeric fields");
        const size_t count = size_t(width * (rows ? rows : 1));
        if (out.size() + count > 4096) throw std::length_error("HLSL aggregate component limit exceeded");
        for (int r = 0; r < (rows ? rows : 1); ++r) for (int c = 0; c < width; ++c)
            out.push_back(value + (rows ? "[" + std::to_string(c) + "][" + std::to_string(r) + "]" :
                width > 1 ? "[" + std::to_string(c) + "]" : ""));
    }
}

static std::string AggregateValue(HLSLTree& tree, const HLSLType& type,
    const std::vector<std::string>& values, size_t& index) {
    DepthGuard depth;
    std::string result = GetTypeName(type);
    std::vector<std::string> elements;
    if (type.array) {
        const int count = AggregateArrayExtent(tree, type);
        result += "[" + std::to_string(count) + "]";
        HLSLType element = type; element.array = false; element.arraySize = NULL;
        for (int i = 0; i < count; ++i) elements.push_back(AggregateValue(tree, element, values, index));
    } else if (type.baseType == HLSLBaseType_UserDefined) {
        auto* structure = tree.FindGlobalStruct(type.typeName);
        if (!structure || !structure->field) throw std::invalid_argument("Invalid HLSL aggregate structure");
        for (auto* field = structure->field; field; field = field->nextField)
            elements.push_back(AggregateValue(tree, field->type, values, index));
    } else {
        const int width = NumericWidth(type.baseType), rows = MatrixRows(type.baseType);
        if (!width) throw std::invalid_argument("HLSL aggregate requires numeric fields");
        const size_t count = size_t(width * (rows ? rows : 1));
        if (index + count > values.size()) throw std::invalid_argument("Incomplete HLSL aggregate value");
        // GLSL matrix constructors consume columns; the input sequence is rows.
        if (rows) {
            const std::string scalar = GetTypeName(HLSLType(ScalarBaseType[type.baseType]));
            for (int c = 0; c < width; ++c) for (int r = 0; r < rows; ++r)
                elements.push_back(scalar + "(" + values[index + size_t(r * width + c)] + ")");
        } else for (size_t i = 0; i < count; ++i) elements.push_back(values[index + i]);
        index += count;
    }
    result += "(";
    for (size_t i = 0; i < elements.size(); ++i) {
        if (i) result += ",";
        result += elements[i];
        if (result.size() > 1024 * 1024) throw std::length_error("HLSL aggregate output limit exceeded");
    }
    return result + ")";
}

void GLSLGenerator::OutputAggregateConstructor(HLSLConstructorExpression* expression) {
    bool direct = !expression->type.array && (IsScalarType(expression->type) || IsVectorType(expression->type));
    for (auto* a = expression->argument; direct && a; a = a->nextExpression)
        direct = !a->expressionType.array && (IsScalarType(a->expressionType) || IsVectorType(a->expressionType));
    if (direct) {
        m_writer.Write("%s(", GetTypeName(expression->type));
        OutputExpressionList(expression->argument); m_writer.Write(")");
        return;
    }
    m_writer.Write("(");
    std::vector<std::string> values;
    for (auto* a = expression->argument; a; a = a->nextExpression) {
        auto* temporary = LocalTemporary(a->expressionType);
        m_writer.Write("%s = ", temporary->name); OutputExpression(a); m_writer.Write(", ");
        AggregateValues(*m_tree, a->expressionType, temporary->name, values);
    }
    size_t index = 0;
    const auto value = AggregateValue(*m_tree, expression->type, values, index);
    if (index != values.size()) throw std::invalid_argument("Excess HLSL aggregate values");
    m_writer.Write("%s)", value.c_str());
}

void GLSLGenerator::OutputAggregateConversion(HLSLExpression* expression, const HLSLType& type) {
    const auto& source = expression->expressionType;
    if (source.baseType == type.baseType && source.array == type.array &&
        (source.baseType != HLSLBaseType_UserDefined || String_Equal(source.typeName, type.typeName)) &&
        (!type.array || AggregateArrayExtent(*m_tree, source) == AggregateArrayExtent(*m_tree, type))) {
        OutputExpression(expression); return;
    }
    std::vector<HLSLBaseType> from, to;
    AggregateScalarTypes(*m_tree, source, from); AggregateScalarTypes(*m_tree, type, to);
    const bool splat = !source.array && IsScalarType(source);
    if (!splat && from.size() < to.size()) throw std::invalid_argument("HLSL aggregate cast has too few components");
    auto* temporary = LocalTemporary(source);
    m_writer.Write("(%s = ", temporary->name); OutputExpression(expression); m_writer.Write(", ");
    std::vector<std::string> values;
    if (splat) values.assign(to.size(), temporary->name);
    else AggregateValues(*m_tree, source, temporary->name, values);
    size_t index = 0;
    m_writer.Write("%s)", AggregateValue(*m_tree, type, values, index).c_str());
}
