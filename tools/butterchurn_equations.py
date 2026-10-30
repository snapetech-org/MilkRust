"""Bounded numeric conversion for Butterchurn's generated equation strings.

This is a parser, not a JavaScript runtime. It accepts scoped numeric state,
local temporaries, indexed EEL memory, numeric expressions and bounded VM control
flow. Objects, dynamic property access, closures, host APIs and downloaded code
execution are unavailable.
"""
import json
import math
import re
from dataclasses import dataclass

MAX_EQUATIONS = 65536
TOKEN = re.compile(r"\s+|//[^\n]*|/\*[\s\S]*?\*/|\"(?:\\.|[^\"\\])*\"|'(?:\\.|[^'\\])*'|(?:0[xX][0-9a-fA-F]+|\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|[A-Za-z_][A-Za-z_0-9]*|>>>=|>>>|===|!==|<<=|>>=|\+\+|--|\+=|-=|\*=|/=|%=|&=|\|=|\^=|==|!=|<=|>=|&&|\|\||<<|>>|[.{}\[\]+*/%()=,;?:<>!~&|^\-]")
IDENT = re.compile(r"^[A-Za-z_][A-Za-z_0-9]{0,127}$")
PRECEDENCE = {",": 1, "=": 2, "+=": 2, "-=": 2, "*=": 2, "/=": 2, "%=": 2, "&=": 2, "|=": 2, "^=": 2, "<<=": 2, ">>=": 2, ">>>=": 2,
              "||": 4, "&&": 5, "|": 6, "^": 7, "&": 8, "==": 9, "!=": 9, "===": 9, "!==": 9,
              "<": 10, ">": 10, "<=": 10, ">=": 10, "<<": 11, ">>": 11, ">>>": 11,
              "+": 12, "-": 12, "*": 13, "/": 13, "%": 13}
CALLS = {name: (name, count) for name, count in {
    "sin": 1, "cos": 1, "tan": 1, "asin": 1, "acos": 1, "atan": 1, "atan2": 2,
    "abs": 1, "sqrt": 1, "exp": 1, "log": 1, "log10": 1, "floor": 1, "ceil": 1,
    "pow": 2, "min": 2, "max": 2, "sign": 1, "above": 2, "below": 2, "equal": 2,
    "ifcond": 3, "rand": 1, "sqr": 1, "int": 1, "sigmoid": 2, "bor": 2, "band": 2, "bnot": 1,
    "div": 2, "mod": 2,
}.items()}
CALLS["ifcond"] = ("if", 3)


class Unsupported(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Unsupported(message)


@dataclass
class Expression:
    code: str
    target: object = None


class Equations:
    def __init__(self, source):
        require(isinstance(source, str) and len(source.encode()) <= MAX_EQUATIONS, "equation source limit")
        self.tokens, offset = [], 0
        while offset < len(source):
            match = TOKEN.match(source, offset)
            require(match is not None, f"unsupported equation token at byte {offset}")
            token = match[0]
            offset = match.end()
            if token.isspace() or token.startswith(("//", "/*")):
                continue
            require(not token.lower().startswith("bcjs"), "reserved converter identifier")
            self.tokens.append(token)
            require(len(self.tokens) <= 16384, "equation token limit")
        self.tokens.append("<end>")
        self.index = self.depth = self.next_temporary = 0
        self.locals = self.declared_locals()
        self.state_names = {}
        self.loops = []

    def declared_locals(self):
        # var is function-scoped even inside a loop/block. Scan declaration
        # names without evaluating or rewriting any initializer.
        names = set()
        for index, token in enumerate(self.tokens[:-1]):
            if token != "var" or (index and self.tokens[index - 1] == "."):
                continue
            cursor, nesting, expect_name = index + 1, 0, True
            while cursor < len(self.tokens):
                item = self.tokens[cursor]
                if expect_name:
                    require(IDENT.fullmatch(item) and item not in ("a", "Math")
                            and not item.lower().startswith("bcjs"), "invalid local variable")
                    names.add(item)
                    require(len(names) <= 2048, "local variable limit")
                    expect_name = False
                elif item in ("(", "[", "{"):
                    nesting += 1
                elif item in (")", "]", "}"):
                    if nesting == 0:
                        break
                    nesting -= 1
                elif nesting == 0 and item in (";", "<end>"):
                    break
                elif nesting == 0 and item == ",":
                    expect_name = True
                cursor += 1
        return names

    def peek(self):
        return self.tokens[self.index]

    def take(self):
        token = self.peek()
        require(token != "<end>", "unexpected end of equation")
        self.index += 1
        return token

    def expect(self, token):
        require(self.take() == token, f"expected {token}")

    def temporary(self):
        self.next_temporary += 1
        require(self.next_temporary <= 2048, "conversion temporary limit")
        return f"bcjsTemp{self.next_temporary}"

    @staticmethod
    def truth(value):
        # JS truth differs from EEL epsilon truth, including NaN and infinities.
        return f"bcjsTruth({value})"

    def sequence(self, items):
        if not items:
            return "0"
        if len(items) == 1:
            return items[0]
        temporary = self.temporary()
        leaves = [*items[:-1], f"assign({temporary},{items[-1]})"]
        def tree(values):
            if len(values) == 1:
                return values[0]
            middle = len(values) // 2
            return f"exec2({tree(values[:middle])},{tree(values[middle:])})"
        return f"if({tree(leaves)},{temporary},{temporary})"

    def address(self, expression):
        require(expression.target is not None, "assignment needs numeric state or a memory cell")
        if isinstance(expression.target, str):
            return [], expression.target
        memory, index = expression.target
        temporary = self.temporary()
        return [f"assign({temporary},{index})"], f"{memory}({temporary})"

    def assignment(self, left, right, operator="="):
        setup, target = self.address(left)
        if operator != "=":
            right = self.binary(operator[:-1], target, right)
        return Expression(self.sequence([*setup, f"assign({target},{right})"]))

    def increment(self, expression, delta, postfix=False):
        setup, target = self.address(expression)
        if not postfix:
            return Expression(self.sequence([*setup, f"assign({target},({target}+({delta})))"]))
        before = self.temporary()
        return Expression(self.sequence([*setup, f"assign({before},{target})",
                                         f"assign({target},({before}+({delta})))", before]))

    def binary(self, operator, left, right):
        if operator in ("&&", "||"):
            temporary = self.temporary()
            test = self.truth(f"assign({temporary},{left})")
            return f"if({test},{right},{temporary})" if operator == "&&" else f"if({test},{temporary},{right})"
        if operator in ("==", "==="):
            return f"bcjsEqual({left},{right})"
        if operator in ("!=", "!=="):
            return f"(1-bcjsEqual({left},{right}))"
        if operator in ("<", ">", "<=", ">="):
            return f"({left}{operator}{right})"
        functions = {"%": "bcjsRemainder", "&": "bcjsBitAnd", "|": "bcjsBitOr", "^": "bcjsBitXor",
                     "<<": "bcjsShiftLeft", ">>": "bcjsShiftRight", ">>>": "bcjsUnsignedShiftRight",
                     "/": "bcjsDivide"}
        if operator in functions:
            return f"{functions[operator]}({left},{right})"
        require(operator in ("+", "-", "*"), "unsupported numeric operation")
        return f"({left}{operator}{right})"

    def expression(self, minimum=1):
        self.depth += 1
        require(self.depth <= 64, "equation nesting limit")
        try:
            token = self.take()
            if token in ("+", "-", "!", "~", "++", "--"):
                operand = self.expression(14)
                if token in ("++", "--"):
                    left = self.increment(operand, 1 if token == "++" else -1)
                elif token == "!":
                    left = Expression(f"(1-{self.truth(operand.code)})")
                elif token == "~":
                    left = Expression(f"bcjsBitNot({operand.code})")
                else:
                    left = Expression(f"({token}{operand.code})")
            elif token == "(":
                left = self.expression()
                self.expect(")")
            elif token == "a":
                if self.peek() == ".":
                    self.take()
                    name = self.take()
                else:
                    self.expect("[")
                    quoted = self.take()
                    require(quoted.startswith(('"', "'")), "dynamic state properties are unsupported")
                    name = json.loads(quoted) if quoted.startswith('"') else quoted[1:-1]
                    self.expect("]")
                require(IDENT.fullmatch(name) and not name.lower().startswith("bcjs"), "invalid scoped variable")
                previous = self.state_names.setdefault(name.lower(), name)
                require(previous == name, "state names collide after EEL case folding")
                if name in ("megabuf", "gmegabuf"):
                    self.expect("[")
                    index = self.expression().code
                    self.expect("]")
                    left = Expression(f"{name}({index})", (name, index))
                else:
                    variable = "bcjsState_" + name.encode().hex() if name.lower() in ("e", "pi") else name
                    left = Expression(variable, variable)
            elif token == "Math":
                self.expect(".")
                name = self.take()
                if name in ("PI", "E", "LN2", "LN10", "LOG2E", "LOG10E", "SQRT2", "SQRT1_2"):
                    value = {"PI": math.pi, "E": math.e, "LN2": math.log(2), "LN10": math.log(10),
                             "LOG2E": math.log2(math.e), "LOG10E": math.log10(math.e),
                             "SQRT2": math.sqrt(2), "SQRT1_2": math.sqrt(.5)}[name]
                    left = Expression(repr(value))
                else:
                    left = Expression(self.call(name, math_call=True))
            elif token in ("true", "false"):
                left = Expression("1" if token == "true" else "0")
            elif token in self.locals:
                local = "bcjsLocal_" + token.encode().hex()
                left = Expression(local, local)
            elif re.fullmatch(r"(?:0[xX][0-9a-fA-F]+|\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?", token):
                value = float(int(token, 16)) if token.lower().startswith("0x") else float(token)
                require(math.isfinite(value), "non-finite numeric literal")
                left = Expression(repr(value) if token.lower().startswith("0x") else token)
            else:
                left = Expression(self.call(token))
            while True:
                operator = self.peek()
                if operator in ("++", "--"):
                    self.take()
                    left = self.increment(left, 1 if operator == "++" else -1, postfix=True)
                    continue
                if operator == "?" and minimum <= 3:
                    self.take()
                    yes = self.expression(2).code
                    self.expect(":")
                    no = self.expression(2).code
                    left = Expression(f"if({self.truth(left.code)},{yes},{no})")
                    continue
                precedence = PRECEDENCE.get(operator, 0)
                if precedence < minimum:
                    break
                self.take()
                right = self.expression(precedence if precedence == 2 else precedence + 1).code
                if precedence == 2:
                    left = self.assignment(left, right, operator)
                elif operator == ",":
                    left = Expression(self.sequence([left.code, right]))
                else:
                    left = Expression(self.binary(operator, left.code, right))
            return left
        finally:
            self.depth -= 1

    def call(self, name, math_call=False):
        math_counts = {
            "abs": 1, "acos": 1, "acosh": 1, "asin": 1, "asinh": 1, "atan": 1, "atanh": 1,
            "atan2": 2, "cbrt": 1, "ceil": 1, "clz32": 1, "cos": 1, "cosh": 1,
            "exp": 1, "expm1": 1, "floor": 1, "fround": 1, "imul": 2, "log": 1, "log10": 1,
            "log1p": 1, "log2": 1, "pow": 2, "round": 1, "sign": 1, "sin": 1,
            "sinh": 1, "sqrt": 1, "tan": 1, "tanh": 1, "trunc": 1,
        }
        supported = (name in math_counts or name in ("min", "max")) if math_call else name in CALLS
        require(supported, f"unsupported equation call: {name}")
        self.expect("(")
        arguments = []
        if self.peek() != ")":
            while True:
                arguments.append(self.expression(2).code)
                require(len(arguments) <= 64, "equation call argument limit")
                if self.peek() != ",":
                    break
                self.take()
        self.expect(")")
        if math_call:
            if name in ("min", "max"):
                if not arguments:
                    return "bcjsDivide(1,0)" if name == "min" else "bcjsDivide(-1,0)"
                value = arguments[0]
                for argument in arguments[1:]:
                    value = f"bcjsMath{name}({value},{argument})"
                return value
            require(len(arguments) == math_counts[name], "invalid Math arity")
            return f"bcjsMath{name}({','.join(arguments)})"
        target, count = CALLS[name]
        require(len(arguments) == count, f"unsupported arity for {name}")
        if name == "ifcond":
            # A JS helper call evaluates every argument before choosing a result.
            temporaries = [self.temporary() for _ in arguments]
            return self.sequence([*(f"assign({t},{v})" for t, v in zip(temporaries, arguments)),
                                  f"if({','.join(temporaries)})"])
        return f"{target}({','.join(arguments)})"

    def declarations(self, semicolon=True):
        self.expect("var")
        values = []
        while True:
            name = self.take()
            require(IDENT.fullmatch(name) and name not in ("a", "Math") and not name.lower().startswith("bcjs"), "invalid local variable")
            self.locals.add(name)
            if self.peek() == "=":
                self.take()
                value = self.expression(2).code
                values.append(f"assign(bcjsLocal_{name.encode().hex()},{value})")
            if self.peek() != ",":
                break
            self.take()
        if semicolon:
            self.expect(";")
        return self.sequence(values)

    def statement(self):
        self.depth += 1
        require(self.depth <= 64, "statement nesting limit")
        try:
            token = self.peek()
            if token == ";":
                self.take()
                return "0"
            if token == "{":
                self.take()
                statements = []
                while self.peek() != "}":
                    require(self.peek() != "<end>", "unclosed equation block")
                    item = self.statement()
                    if self.loops:
                        stop, skip = self.loops[-1]
                        item = f"if(({stop}+{skip}),0,{item})"
                    statements.append(item)
                self.take()
                return self.sequence(statements)
            if token == "var":
                return self.declarations()
            if token == "if":
                self.take()
                self.expect("(")
                condition = self.expression().code
                self.expect(")")
                yes = self.statement()
                no = "0"
                if self.peek() == "else":
                    self.take()
                    no = self.statement()
                return f"if({self.truth(condition)},{yes},{no})"
            if token in ("for", "while", "do"):
                self.take()
                stop, skip = self.temporary(), self.temporary()
                self.loops.append((stop, skip))
                initial, update = "0", "0"
                if token == "do":
                    body = self.statement()
                    self.expect("while")
                    self.expect("(")
                    condition = self.expression().code
                    self.expect(")")
                    self.expect(";")
                    condition = self.sequence([f"assign({skip},0)", body,
                                               f"if({stop},0,{self.truth(condition)})"])
                    loop = f"while({condition})"
                else:
                    self.expect("(")
                    if token == "for":
                        if self.peek() == "var":
                            initial = self.declarations()
                        else:
                            initial = "0" if self.peek() == ";" else self.expression().code
                            self.expect(";")
                        condition = "1" if self.peek() == ";" else self.expression().code
                        self.expect(";")
                        update = "0" if self.peek() == ")" else self.expression().code
                    else:
                        condition = self.expression().code
                    self.expect(")")
                    body = self.statement()
                    step = self.sequence([f"assign({skip},0)", body, f"if({stop},0,{update})"])
                    loop = f"while(if({stop},0,{self.truth(condition)}),{step})"
                self.loops.pop()
                return self.sequence([f"assign({stop},0)", f"assign({skip},0)", initial, loop])
            if token in ("break", "continue"):
                require(self.loops, "loop control outside a loop")
                self.take()
                self.expect(";")
                return f"assign({self.loops[-1][0 if token == 'break' else 1]},1)"
            expression = self.expression().code
            if self.peek() == ";":
                self.take()
            else:
                require(self.peek() in ("}", "<end>"), "unsupported equation statement")
            return expression
        finally:
            self.depth -= 1

    def statements(self):
        # Uninitialized numeric locals begin as NaN on every invocation.
        # A bare var inside a loop must not reset the previous iteration's value.
        output = [f"assign(bcjsLocal_{name.encode().hex()},bcjsDivide(0,0));"
                  for name in sorted(self.locals)]
        while self.peek() != "<end>":
            output.append(self.statement() + ";")
        require(sum(len(line.encode()) for line in output) <= 256 * 1024, "converted equation byte limit")
        return output
