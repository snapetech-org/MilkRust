// Bounded scalar constant evaluation for declarations, array extents and hints.
// Values carry the HLSL type: uint arithmetic wraps at 32 bits, signed overflow
// is rejected, and floating intermediates round to the profile's float32.
struct ScalarConstant {
    HLSLBaseType type;
    double value;
    ScalarConstant(HLSLBaseType t = HLSLBaseType_Int, double v = 0) : type(t), value(v) {}
};
static bool ConvertConstant(ScalarConstant& value, HLSLBaseType type) {
    if (!IsScalarType(type)) return false;
    if (!std::isfinite(value.value)) throw std::range_error("Non-finite HLSL constant");
    if (type == HLSLBaseType_Bool) value.value = value.value != 0;
    else if (type == HLSLBaseType_Float || type == HLSLBaseType_Half) {
        value.value = static_cast<float>(value.value);
        if (!std::isfinite(value.value)) throw std::range_error("HLSL floating constant overflow");
    } else if (type == HLSLBaseType_Int || type == HLSLBaseType_Uint) {
        const double number = std::trunc(value.value);
        if (value.type == HLSLBaseType_Float || value.type == HLSLBaseType_Half) {
            const double minimum = type == HLSLBaseType_Int ? -2147483648.0 : 0.0;
            const double maximum = type == HLSLBaseType_Int ? 2147483647.0 : 4294967295.0;
            if (number < minimum || number > maximum) throw std::range_error("HLSL floating to integer constant overflow");
        }
        if (number < -2147483648.0 || number > 4294967295.0) throw std::range_error("HLSL integer constant overflow");
        const auto bits = static_cast<std::uint32_t>(static_cast<std::int64_t>(number));
        value.value = type == HLSLBaseType_Uint ? double(bits) :
            bits <= 0x7fffffffu ? double(bits) : double(std::int64_t(bits) - 0x100000000ll);
    } else return false;
    value.type = type;
    return true;
}

static bool EvaluateScalar(HLSLTree& tree, HLSLExpression* expression, ScalarConstant& result);
static bool EvaluateComponent(HLSLTree& tree, HLSLExpression* expression, size_t component, ScalarConstant& result);

static bool EvaluateScalar(HLSLTree& tree, HLSLExpression* expression, ScalarConstant& result) {
    DepthGuard depth;
    if (!expression || !(expression->expressionType.flags & HLSLTypeFlag_Const) ||
        expression->expressionType.array || !IsScalarType(expression->expressionType)) return false;
    const auto output = expression->expressionType.baseType;
    if (expression->nodeType == HLSLNodeType_MemberAccess || expression->nodeType == HLSLNodeType_ArrayAccess)
        return EvaluateComponent(tree, expression, 0, result) && ConvertConstant(result, output);
    if (expression->nodeType == HLSLNodeType_LiteralExpression) {
        const auto* literal = static_cast<HLSLLiteralExpression*>(expression);
        result.type = output;
        if (output == HLSLBaseType_Int) result.value = literal->iValue;
        else if (output == HLSLBaseType_Uint) result.value = static_cast<std::uint32_t>(literal->iValue);
        else if (output == HLSLBaseType_Bool) result.value = literal->bValue;
        else result.value = literal->fValue;
        return ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_IdentifierExpression) {
        const auto* identifier = static_cast<HLSLIdentifierExpression*>(expression);
        const auto* declaration = identifier->declaration ? identifier->declaration : tree.FindGlobalDeclaration(identifier->name);
        if (!declaration || !(declaration->type.flags & HLSLTypeFlag_Const) ||
            !EvaluateScalar(tree, declaration->assignment, result)) return false;
        return ConvertConstant(result, declaration->type.baseType);
    }
    if (expression->nodeType == HLSLNodeType_CastingExpression) {
        auto* cast = static_cast<HLSLCastingExpression*>(expression);
        return EvaluateComponent(tree, cast->expression, 0, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_ConstructorExpression) {
        auto* constructor = static_cast<HLSLConstructorExpression*>(expression);
        if (!constructor->argument || constructor->argument->nextExpression) return false;
        return EvaluateComponent(tree, constructor->argument, 0, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_ConditionalExpression) {
        auto* conditional = static_cast<HLSLConditionalExpression*>(expression);
        ScalarConstant condition;
        if (!EvaluateScalar(tree, conditional->condition, condition)) return false;
        return EvaluateScalar(tree, condition.value != 0 ? conditional->trueExpression : conditional->falseExpression, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_UnaryExpression) {
        auto* unary = static_cast<HLSLUnaryExpression*>(expression);
        if (!EvaluateScalar(tree, unary->expression, result)) return false;
        switch (unary->unaryOp) {
            case HLSLUnaryOp_Positive: return ConvertConstant(result, output);
            case HLSLUnaryOp_Not: result = ScalarConstant(HLSLBaseType_Bool, result.value == 0); return true;
            case HLSLUnaryOp_Negative:
                if (output == HLSLBaseType_Uint) result.value = std::uint32_t(0u - static_cast<std::uint32_t>(result.value));
                else if (output == HLSLBaseType_Int) result.value = CheckedConstant(-static_cast<std::int64_t>(result.value));
                else result.value = -result.value;
                return ConvertConstant(result, output);
            case HLSLUnaryOp_BitNot: {
                const auto bits = ~static_cast<std::uint32_t>(static_cast<std::int64_t>(result.value));
                result = ScalarConstant(HLSLBaseType_Uint, bits); return ConvertConstant(result, output);
            }
            default: return false;
        }
    }
    if (expression->nodeType != HLSLNodeType_BinaryExpression) return false;
    auto* binary = static_cast<HLSLBinaryExpression*>(expression);
    ScalarConstant a, b;
    if (!EvaluateScalar(tree, binary->expression1, a) || !EvaluateScalar(tree, binary->expression2, b)) return false;
    const auto op = binary->binaryOp;
    if (op == HLSLBinaryOp_And || op == HLSLBinaryOp_Or) {
        result = ScalarConstant(HLSLBaseType_Bool, op == HLSLBinaryOp_And ? (a.value != 0 && b.value != 0) : (a.value != 0 || b.value != 0));
        return true;
    }
    const auto common = CommonNumericType(a.type, b.type);
    if (op != HLSLBinaryOp_LeftShift && op != HLSLBinaryOp_RightShift) {
        if (!ConvertConstant(a, common) || !ConvertConstant(b, common)) return false;
    }
    switch (op) {
        case HLSLBinaryOp_Less: result = ScalarConstant(HLSLBaseType_Bool, a.value < b.value); return true;
        case HLSLBinaryOp_Greater: result = ScalarConstant(HLSLBaseType_Bool, a.value > b.value); return true;
        case HLSLBinaryOp_LessEqual: result = ScalarConstant(HLSLBaseType_Bool, a.value <= b.value); return true;
        case HLSLBinaryOp_GreaterEqual: result = ScalarConstant(HLSLBaseType_Bool, a.value >= b.value); return true;
        case HLSLBinaryOp_Equal: result = ScalarConstant(HLSLBaseType_Bool, a.value == b.value); return true;
        case HLSLBinaryOp_NotEqual: result = ScalarConstant(HLSLBaseType_Bool, a.value != b.value); return true;
        default: break;
    }
    if (output == HLSLBaseType_Float || output == HLSLBaseType_Half) {
        double value = 0;
        switch (op) {
            case HLSLBinaryOp_Add: value = a.value + b.value; break;
            case HLSLBinaryOp_Sub: value = a.value - b.value; break;
            case HLSLBinaryOp_Mul: value = a.value * b.value; break;
            case HLSLBinaryOp_Div:
            case HLSLBinaryOp_Mod:
                if (b.value == 0) throw std::range_error("HLSL constant division by zero");
                value = op == HLSLBinaryOp_Div ? a.value / b.value : std::fmod(a.value, b.value); break;
            default: return false;
        }
        result = ScalarConstant(output, value); return ConvertConstant(result, output);
    }
    if (output != HLSLBaseType_Int && output != HLSLBaseType_Uint) return false;
    const auto left = static_cast<std::int64_t>(a.value), right = static_cast<std::int64_t>(b.value);
    const bool isUnsigned = output == HLSLBaseType_Uint;
    std::int64_t value = 0;
    switch (op) {
        case HLSLBinaryOp_Add: value = left + right; break;
        case HLSLBinaryOp_Sub: value = left - right; break;
        case HLSLBinaryOp_Mul:
            value = isUnsigned ? static_cast<std::uint32_t>(std::uint64_t(left) * std::uint64_t(right)) : left * right; break;
        case HLSLBinaryOp_Div:
        case HLSLBinaryOp_Mod:
            if (!right) throw std::range_error("HLSL constant division by zero");
            value = op == HLSLBinaryOp_Div ? left / right : left % right; break;
        case HLSLBinaryOp_LeftShift:
        case HLSLBinaryOp_RightShift:
            if (right < 0 || right > 31) throw std::range_error("HLSL constant shift is out of range");
            if (op == HLSLBinaryOp_LeftShift)
                value = isUnsigned ? std::uint32_t(std::uint64_t(left) << right) : left * (std::int64_t(1) << right);
            else value = left >= 0 ? left / (std::int64_t(1) << right) : -1 - ((-1 - left) / (std::int64_t(1) << right));
            break;
        case HLSLBinaryOp_BitAnd: value = left & right; break;
        case HLSLBinaryOp_BitOr: value = left | right; break;
        case HLSLBinaryOp_BitXor: value = left ^ right; break;
        default: return false;
    }
    result = ScalarConstant(output, isUnsigned ? double(static_cast<std::uint32_t>(value)) : double(CheckedConstant(value)));
    return true;
}

static HLSLLiteralExpression ConstantLiteral(const ScalarConstant& value) {
    HLSLLiteralExpression literal;
    literal.nodeType = HLSLNodeType_LiteralExpression;
    literal.fileName = NULL; literal.line = 0;
    literal.type = value.type; literal.expressionType = HLSLType(value.type);
    literal.expressionType.flags = HLSLTypeFlag_Const;
    if (value.type == HLSLBaseType_Bool) literal.bValue = value.value != 0;
    else if (value.type == HLSLBaseType_Float || value.type == HLSLBaseType_Half) literal.fValue = static_cast<float>(value.value);
    else {
        const auto bits = static_cast<std::uint32_t>(static_cast<std::int64_t>(value.value));
        literal.iValue = int(bits <= 0x7fffffffu ? std::int64_t(bits) : std::int64_t(bits) - 0x100000000ll);
    }
    return literal;
}

static size_t ConstantComponentCount(HLSLTree& tree, const HLSLType& type) {
    std::vector<HLSLBaseType> components;
    AggregateScalarTypes(tree, type, components);
    return components.size();
}

static size_t ConstantCastComponent(const HLSLType& source, const HLSLType& target, size_t index) {
    if (!source.array && IsScalarType(source)) return 0;
    if (!source.array && !target.array && IsMatrixType(source.baseType) && IsMatrixType(target.baseType)) {
        const size_t row = index / size_t(NumericWidth(target.baseType));
        const size_t column = index % size_t(NumericWidth(target.baseType));
        if (row >= size_t(MatrixRows(source.baseType)) || column >= size_t(NumericWidth(source.baseType)))
            throw std::invalid_argument("Invalid constant matrix cast shape");
        return row * size_t(NumericWidth(source.baseType)) + column;
    }
    return index;
}

static bool EvaluateComponent(HLSLTree& tree, HLSLExpression* expression, size_t component, ScalarConstant& result) {
    DepthGuard depth;
    if (!expression || !(expression->expressionType.flags & HLSLTypeFlag_Const)) return false;
    const auto& type = expression->expressionType;
    if (!type.array && IsScalarType(type) && expression->nodeType != HLSLNodeType_MemberAccess &&
        expression->nodeType != HLSLNodeType_ArrayAccess)
        return component == 0 && EvaluateScalar(tree, expression, result);
    std::vector<HLSLBaseType> components;
    AggregateScalarTypes(tree, type, components);
    if (component >= components.size()) throw std::invalid_argument("HLSL constant component is out of bounds");
    const auto output = components[component];
    if (expression->nodeType == HLSLNodeType_IdentifierExpression) {
        auto* identifier = static_cast<HLSLIdentifierExpression*>(expression);
        auto* declaration = identifier->declaration ? identifier->declaration : tree.FindGlobalDeclaration(identifier->name);
        return declaration && (declaration->type.flags & HLSLTypeFlag_Const) &&
            EvaluateComponent(tree, declaration->assignment, component, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_ConstructorExpression) {
        auto* constructor = static_cast<HLSLConstructorExpression*>(expression);
        if (!constructor->initializer && constructor->argument && !constructor->argument->nextExpression) {
            auto* argument = constructor->argument;
            return EvaluateComponent(tree, argument, ConstantCastComponent(argument->expressionType, type, component), result) && ConvertConstant(result, output);
        }
        for (auto* argument = constructor->argument; argument; argument = argument->nextExpression) {
            const size_t count = ConstantComponentCount(tree, argument->expressionType);
            if (component < count) return EvaluateComponent(tree, argument, component, result) && ConvertConstant(result, output);
            component -= count;
        }
        return false;
    }
    if (expression->nodeType == HLSLNodeType_CastingExpression) {
        auto* cast = static_cast<HLSLCastingExpression*>(expression);
        return EvaluateComponent(tree, cast->expression, ConstantCastComponent(cast->expression->expressionType, type, component), result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_MemberAccess) {
        auto* member = static_cast<HLSLMemberAccess*>(expression);
        const auto& object = member->object->expressionType;
        size_t selected = 0;
        if (!object.array && object.baseType == HLSLBaseType_UserDefined) {
            auto* structure = tree.FindGlobalStruct(object.typeName);
            auto* field = structure ? structure->field : NULL;
            for (; field && !String_Equal(field->name, member->field); field = field->nextField)
                selected += ConstantComponentCount(tree, field->type);
            if (!field) return false;
            selected += component;
        } else if (!object.array && IsMatrixType(object.baseType)) {
            const char* p = member->field;
            for (size_t i = 0; i <= component; ++i) {
                if (*p++ != '_') return false;
                const int base = *p == 'm' ? (++p, '0') : '1';
                if (!p[0] || !p[1]) return false;
                const int row = p[0] - base, column = p[1] - base;
                if (row < 0 || column < 0 || row >= MatrixRows(object.baseType) || column >= NumericWidth(object.baseType)) return false;
                selected = size_t(row * NumericWidth(object.baseType) + column); p += 2;
            }
        } else if (!object.array && (IsScalarType(object) || IsVectorType(object))) {
            if (component >= strlen(member->field)) return false;
            const char* alphabet = "xyzw";
            const char* found = strchr(alphabet, member->field[component]);
            if (!found) { alphabet = "rgba"; found = strchr(alphabet, member->field[component]); }
            if (!found) return false;
            selected = size_t(found - alphabet);
        } else return false;
        return EvaluateComponent(tree, member->object, selected, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_ArrayAccess) {
        auto* access = static_cast<HLSLArrayAccess*>(expression);
        ScalarConstant index;
        if (!EvaluateScalar(tree, access->index, index) || !ConvertConstant(index, HLSLBaseType_Int)) return false;
        const auto& array = access->array->expressionType;
        const int extent = array.array ? AggregateArrayExtent(tree, array) :
            IsMatrixType(array.baseType) ? MatrixRows(array.baseType) : NumericWidth(array.baseType);
        if (index.value < 0 || index.value >= extent) throw std::invalid_argument("HLSL constant index is out of bounds");
        return EvaluateComponent(tree, access->array, size_t(index.value) * components.size() + component, result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_ConditionalExpression) {
        auto* conditional = static_cast<HLSLConditionalExpression*>(expression);
        ScalarConstant condition;
        const auto index = IsScalarType(conditional->condition->expressionType) ? 0 : component;
        if (!EvaluateComponent(tree, conditional->condition, index, condition)) return false;
        auto* selected = condition.value != 0 ? conditional->trueExpression : conditional->falseExpression;
        return EvaluateComponent(tree, selected, ConstantCastComponent(selected->expressionType, type, component), result) && ConvertConstant(result, output);
    }
    if (expression->nodeType == HLSLNodeType_UnaryExpression) {
        auto unary = *static_cast<HLSLUnaryExpression*>(expression);
        ScalarConstant operand;
        if (!EvaluateComponent(tree, unary.expression, component, operand)) return false;
        auto literal = ConstantLiteral(operand); unary.expression = &literal;
        unary.expressionType = HLSLType(output); unary.expressionType.flags = HLSLTypeFlag_Const;
        return EvaluateScalar(tree, &unary, result);
    }
    if (expression->nodeType == HLSLNodeType_BinaryExpression) {
        auto binary = *static_cast<HLSLBinaryExpression*>(expression);
        ScalarConstant a, b;
        if (!EvaluateComponent(tree, binary.expression1, ConstantCastComponent(binary.expression1->expressionType, type, component), a) ||
            !EvaluateComponent(tree, binary.expression2, ConstantCastComponent(binary.expression2->expressionType, type, component), b)) return false;
        auto left = ConstantLiteral(a), right = ConstantLiteral(b);
        binary.expression1 = &left; binary.expression2 = &right;
        binary.expressionType = HLSLType(output); binary.expressionType.flags = HLSLTypeFlag_Const;
        return EvaluateScalar(tree, &binary, result);
    }
    return false;
}
