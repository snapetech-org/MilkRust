"""Convert Butterchurn's generated GLSL to the bounded typed HLSL profile.

GLSL column vectors are represented as HLSL rows. Matrix multiplication reverses
its operands inside typed helpers; ordinary scalar/vector multiplication remains
componentwise. No shader is executed, and no source is passed through unchecked.
"""
import re

MAX_SHADER = 128 * 1024
TOKEN = re.compile(r"\s+|//[^\n]*|/\*[\s\S]*?\*/|(?:0[xX][0-9a-fA-F]+|\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?[fFuU]?|[A-Za-z_][A-Za-z_0-9]*|<<=|>>=|\+\+|--|==|!=|<=|>=|&&|\|\||<<|>>|\+=|-=|\*=|/=|%=|&=|\|=|\^=|[{}()\[\].,;?:+\-*/%<>=!~&|^]")
IDENT = re.compile(r"^[A-Za-z_][A-Za-z_0-9]*$")
TYPES = {"void": "void", "float": "float", "int": "int", "uint": "uint", "bool": "bool",
         "sampler2D": "sampler2D", "sampler3D": "sampler3D"}
for prefix, target in (("vec", "float"), ("ivec", "int"), ("uvec", "uint"), ("bvec", "bool")):
    for width in range(2, 5):
        TYPES[f"{prefix}{width}"] = f"{target}{width}"
MATRICES = {}
for columns in range(2, 5):
    for rows in range(2, 5):
        name = f"mat{columns}" if columns == rows else f"mat{columns}x{rows}"
        MATRICES[name] = (columns, rows)
        MATRICES[f"mat{columns}x{rows}"] = (columns, rows)
for name, (columns, rows) in MATRICES.items():
    TYPES[name] = f"float{columns}x{rows}"
QUALIFIERS = {"const", "in", "out", "inout", "uniform", "lowp", "mediump", "highp", "precise"}
PRECEDENCE = {"=": 1, "+=": 1, "-=": 1, "*=": 1, "/=": 1, "%=": 1, "&=": 1, "|=": 1, "^=": 1,
              "<<=": 1, ">>=": 1, "||": 3, "&&": 4, "|": 5, "^": 6, "&": 7,
              "==": 8, "!=": 8, "<": 9, ">": 9, "<=": 9, ">=": 9, "<<": 10, ">>": 10,
              "+": 11, "-": 11, "*": 12, "/": 12, "%": 12}


class ShaderConversionError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ShaderConversionError(message)


class Shader:
    def __init__(self, source):
        require(isinstance(source, str) and len(source.encode()) <= MAX_SHADER, "GLSL source exceeds 128 KiB")
        self.tokens, offset = [], 0
        while offset < len(source):
            match = TOKEN.match(source, offset)
            require(match is not None, f"unsupported GLSL token at byte {offset}")
            text = match[0]
            offset = match.end()
            if text.isspace() or text.startswith(("//", "/*")):
                continue
            require(not text.lower().startswith("bcglsl"), "reserved converter identifier")
            self.tokens.append(text)
            require(len(self.tokens) <= 32768, "GLSL token limit")
        self.tokens.append("<end>")
        self.index, self.depth = 0, 0
        self.types = dict(TYPES)
        self.samplers = {}
        self.used = set()
        for index, token in enumerate(self.tokens[:-2]):
            if token in ("sampler2D", "sampler3D") and IDENT.fullmatch(self.tokens[index + 1]):
                self.samplers[self.tokens[index + 1]] = 3 if token == "sampler3D" else 2

    def peek(self):
        return self.tokens[self.index]

    def take(self):
        token = self.peek()
        require(token != "<end>", "unexpected end of GLSL")
        self.index += 1
        return token

    def expect(self, token):
        require(self.take() == token, f"expected GLSL {token}")

    def identifier(self):
        name = self.take()
        require(IDENT.fullmatch(name), "expected GLSL identifier")
        return name

    def enter(self):
        self.depth += 1
        require(self.depth <= 64, "GLSL nesting limit")

    def qualified_type(self):
        qualifiers = []
        uniform = False
        while self.peek() in QUALIFIERS:
            qualifier = self.take()
            uniform = uniform or qualifier == "uniform"
            if qualifier not in ("uniform", "lowp", "mediump", "highp", "precise"):
                qualifiers.append(qualifier)
        name = self.take()
        require(name in self.types, f"unsupported GLSL type {name}")
        require(not uniform or name in ("sampler2D", "sampler3D"), "custom numeric uniforms have no host binding")
        return " ".join([*qualifiers, self.types[name]])

    def declaration(self, terminator=";"):
        type_name = self.qualified_type()
        name = self.identifier()
        if self.peek() == "(":
            self.take()
            parameters = []
            if self.peek() != ")":
                while True:
                    parameter_type = self.qualified_type()
                    if parameter_type == "void" and self.peek() == ")" and not parameters:
                        break
                    parameter = parameter_type + " " + self.identifier()
                    while self.peek() == "[":
                        self.take()
                        parameter += "[" + self.expression() + "]"
                        self.expect("]")
                    parameters.append(parameter)
                    if self.peek() != ",":
                        break
                    self.take()
            self.expect(")")
            signature = f"{type_name} {name}({','.join(parameters)})"
            if self.peek() == ";":
                self.take()
                return signature + ";"
            require(self.peek() == "{", "expected GLSL function body")
            return signature + self.statement()
        declarations = []
        while True:
            current = name
            while self.peek() == "[":
                self.take()
                current += "[" + self.expression() + "]"
                self.expect("]")
            if self.peek() == "=":
                self.take()
                current += "=" + self.expression()
            declarations.append(current)
            if self.peek() != ",":
                break
            self.take()
            name = self.identifier()
        self.expect(terminator)
        return type_name + " " + ",".join(declarations) + terminator

    def statement(self):
        self.enter()
        try:
            token = self.peek()
            if token == "{":
                self.take()
                statements = []
                while self.peek() != "}":
                    require(self.peek() != "<end>", "unclosed GLSL block")
                    statements.append(self.statement())
                self.take()
                return "{\n" + "\n".join(statements) + "\n}"
            if token == "shader_body":
                self.take()
                require(self.peek() == "{", "expected shader_body block")
                body = self.statement()
                # Butterchurn defines these coordinates before running either
                # stage; MilkDrop's default radius/angle conventions differ.
                initialization = ("rad=length(uv_orig-float2(0.5,0.5));"
                                  "ang=atan2(uv_orig.x-0.5,uv_orig.y-0.5);")
                return "shader_body {\n" + initialization + body[1:]
            if token == "precision":
                self.take()
                require(self.take() in ("lowp", "mediump", "highp"), "invalid precision")
                require(self.take() in ("float", "int", "sampler2D", "sampler3D"), "invalid precision type")
                self.expect(";")
                return ""
            if token == "struct":
                self.take()
                name = self.identifier()
                require(name not in self.types, "duplicate GLSL structure")
                self.types[name] = name
                self.expect("{")
                fields = []
                while self.peek() != "}":
                    fields.append(self.declaration())
                self.take()
                self.expect(";")
                return f"struct {name}" + "{\n" + "\n".join(fields) + "\n};"
            if token in ("if", "while", "switch"):
                self.take()
                self.expect("(")
                condition = self.expression()
                self.expect(")")
                result = f"{token}({condition})" + self.statement()
                if token == "if" and self.peek() == "else":
                    self.take()
                    result += " else " + self.statement()
                return result
            if token == "for":
                self.take()
                self.expect("(")
                if self.peek() == ";":
                    self.take()
                    initial = ";"
                elif self.peek() in self.types or self.peek() in QUALIFIERS:
                    initial = self.declaration()
                else:
                    initial = self.expression() + ";"
                    self.expect(";")
                condition = "" if self.peek() == ";" else self.expression()
                self.expect(";")
                step = "" if self.peek() == ")" else self.expression()
                self.expect(")")
                return f"for({initial}{condition};{step})" + self.statement()
            if token == "do":
                self.take()
                body = self.statement()
                self.expect("while")
                self.expect("(")
                condition = self.expression()
                self.expect(")")
                self.expect(";")
                return f"do {body} while({condition});"
            if token in ("break", "continue", "discard"):
                self.take()
                self.expect(";")
                return token + ";"
            if token == "return":
                self.take()
                result = "" if self.peek() == ";" else " " + self.expression()
                self.expect(";")
                return "return" + result + ";"
            if token in ("case", "default"):
                self.take()
                value = "" if token == "default" else " " + self.expression()
                self.expect(":")
                return token + value + ":"
            if token == ";":
                self.take()
                return ";"
            if token in self.types or token in QUALIFIERS:
                return self.declaration()
            result = self.expression()
            self.expect(";")
            return result + ";"
        finally:
            self.depth -= 1

    def expression(self, minimum=1):
        self.enter()
        try:
            token = self.take()
            if token in ("+", "-", "!", "~", "++", "--"):
                left = "(" + token + self.expression(13) + ")"
            elif token == "(":
                left = "(" + self.expression() + ")"
                self.expect(")")
            elif IDENT.fullmatch(token):
                left = token
                if self.peek() == "(":
                    self.take()
                    arguments = []
                    if self.peek() != ")":
                        while True:
                            arguments.append(self.expression())
                            if self.peek() != ",":
                                break
                            self.take()
                    self.expect(")")
                    left = self.call(token, arguments)
            else:
                require(re.fullmatch(r"(?:0[xX][0-9a-fA-F]+|\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?[fFuU]?", token), "invalid GLSL expression")
                left = token
            while True:
                token = self.peek()
                if token == ".":
                    self.take()
                    left += "." + self.identifier()
                    continue
                if token == "[":
                    self.take()
                    left += "[" + self.expression() + "]"
                    self.expect("]")
                    continue
                if token in ("++", "--"):
                    left += self.take()
                    continue
                if token == "?" and minimum <= 2:
                    self.take()
                    yes = self.expression()
                    self.expect(":")
                    no = self.expression(2)
                    left = f"({left}?{yes}:{no})"
                    continue
                precedence = PRECEDENCE.get(token, 0)
                if precedence < minimum:
                    break
                self.take()
                right = self.expression(precedence if precedence == 1 else precedence + 1)
                if token in ("==", "!="):
                    self.used.add("equality")
                    comparison = f"bcglslIsEqual({left},{right})"
                    left = comparison if token == "==" else f"(!{comparison})"
                elif token in ("*", "*="):
                    self.used.add("multiply")
                    function = "bcglslMulAssign" if token == "*=" else "bcglslMul"
                    left = f"{function}({left},{right})"
                else:
                    left = f"({left}{token}{right})"
            return left
        finally:
            self.depth -= 1

    def call(self, name, arguments):
        if name in MATRICES and len(arguments) == 1:
            self.used.add("matrix")
            columns, rows = MATRICES[name]
            name = f"bcglslMatrix{columns}x{rows}"
        elif name in self.types:
            name = self.types[name]
        elif name in ("texture", "texture2D", "texture3D", "textureLod", "texture2DLod", "texture3DLod", "textureGrad"):
            require(arguments and IDENT.fullmatch(arguments[0]), "texture argument must be a static sampler")
            sampler = arguments[0]
            dimension = self.samplers.get(sampler, 3 if re.fullmatch(r"sampler_(?:[pf][wc])?noisevol_(?:lq|hq)", sampler, re.I) else 2)
            if name in ("texture3D", "texture3DLod"):
                dimension = 3
            suffix = "Grad" if name == "textureGrad" else "Lod" if name.endswith("Lod") else "Bias" if len(arguments) == 3 else ""
            require(len(arguments) == (4 if suffix == "Grad" else 3 if suffix else 2), "invalid texture overload")
            self.used.add("texture")
            name = f"bcglslTexture{dimension}D{suffix}"
        elif name == "mod":
            self.used.add("mod")
            name = "bcglslMod"
        elif name == "mix":
            self.used.add("mix")
            name = "bcglslMix"
        elif name in ("equal", "notEqual", "lessThan", "lessThanEqual", "greaterThan", "greaterThanEqual", "not"):
            self.used.add("compare")
            name = "bcglsl" + name[0].upper() + name[1:]
        elif name == "atan" and len(arguments) == 2:
            name = "atan2"
        else:
            name = {"fract": "frac", "inversesqrt": "rsqrt", "dFdx": "ddx", "dFdy": "ddy", "roundEven": "round"}.get(name, name)
        return name + "(" + ",".join(arguments) + ")"

    def convert(self):
        output = []
        while self.peek() != "<end>":
            output.append(self.statement())
        require(sum(token == "shader_body" for token in self.tokens) == 1, "converted shader needs exactly one shader_body")
        environment = ["static const float PI=3.141592653589793;"]
        for level in range(1, 4):
            environment += [f"float scale{level}=blur{level}_max-blur{level}_min;",
                            f"float bias{level}=blur{level}_min;"]
        result = "\n".join(environment) + "\n" + helpers(self.used) + "\n" + "\n".join(output)
        require(len(result.encode()) <= MAX_SHADER, "translated shader exceeds 128 KiB")
        return result


def helpers(used):
    result = []
    seen = set()
    def multiply(left, right, output, expression):
        signature = (left, right)
        if signature in seen:
            return
        seen.add(signature)
        result.append(f"{output} bcglslMul({left} a,{right} b){{return {expression};}}")
        if output == left:
            result.append(f"{left} bcglslMulAssign(inout {left} a,{right} b){{a={expression};return a;}}")
    matrices = [(c, r, f"float{c}x{r}") for c in range(2, 5) for r in range(2, 5)]
    if "multiply" in used:
        for base in ("float", "int", "uint"):
            multiply(base, base, base, "a*b")
            for size in range(2, 5):
                vector = f"{base}{size}"
                multiply(vector, vector, vector, "a*b")
                multiply(vector, base, vector, "a*b")
                multiply(base, vector, vector, "a*b")
        for c, r, matrix in matrices:
            multiply(matrix, "float", matrix, "a*b")
            multiply("float", matrix, matrix, "a*b")
            multiply(matrix, f"float{c}", f"float{r}", "mul(b,a)")
            multiply(f"float{r}", matrix, f"float{c}", "mul(b,a)")
            for output_columns in range(2, 5):
                multiply(matrix, f"float{output_columns}x{c}", f"float{output_columns}x{r}", "mul(b,a)")
    if "matrix" in used:
        for c, r, output in matrices:
            diagonal = ",".join("a" if i == j else "0.0" for i in range(c) for j in range(r))
            result.append(f"{output} bcglslMatrix{c}x{r}(float a){{return {output}({diagonal});}}")
            if c == r == 2:
                result.append("float2x2 bcglslMatrix2x2(float4 a){return float2x2(a.x,a.y,a.z,a.w);}")
            for ic, ir, input_type in matrices:
                components = ",".join(f"a[{i}][{j}]" if i < ic and j < ir else "1.0" if i == j else "0.0"
                                      for i in range(c) for j in range(r))
                result.append(f"{output} bcglslMatrix{c}x{r}({input_type} a){{return {output}({components});}}")
    for size in range(1, 5):
        value = "float" if size == 1 else f"float{size}"
        boolean = "bool" if size == 1 else f"bool{size}"
        if "mod" in used:
            result.append(f"{value} bcglslMod({value} a,{value} b){{return a-b*floor(a/b);}}")
            if size > 1:
                result.append(f"{value} bcglslMod({value} a,float b){{return a-b*floor(a/b);}}")
        if "mix" in used:
            result.append(f"{value} bcglslMix({value} a,{value} b,{value} t){{return lerp(a,b,t);}}")
            result.append(f"{value} bcglslMix({value} a,{value} b,{boolean} t){{return t?b:a;}}")
            if size > 1:
                result.append(f"{value} bcglslMix({value} a,{value} b,float t){{return lerp(a,b,t);}}")
        if "compare" in used:
            for family in ("float", "int", "uint"):
                numeric = family if size == 1 else f"{family}{size}"
                for name, operator in (("Equal", "=="), ("NotEqual", "!="), ("LessThan", "<"), ("LessThanEqual", "<="),
                                       ("GreaterThan", ">"), ("GreaterThanEqual", ">=")):
                    result.append(f"{boolean} bcglsl{name}({numeric} a,{numeric} b){{return a{operator}b;}}")
            result.append(f"{boolean} bcglslEqual({boolean} a,{boolean} b){{return a==b;}}")
            result.append(f"{boolean} bcglslNotEqual({boolean} a,{boolean} b){{return a!=b;}}")
            result.append(f"{boolean} bcglslNot({boolean} a){{return !a;}}")
        if "equality" in used:
            for family in ("float", "int", "uint", "bool"):
                numeric = family if size == 1 else f"{family}{size}"
                comparison = "a==b" if size == 1 else "all(a==b)"
                result.append(f"bool bcglslIsEqual({numeric} a,{numeric} b){{return {comparison};}}")
    if "equality" in used:
        for c, r, matrix in matrices:
            comparison = "&&".join(f"(a[{i}][{j}]==b[{i}][{j}])" for i in range(c) for j in range(r))
            result.append(f"bool bcglslIsEqual({matrix} a,{matrix} b){{return {comparison};}}")
    if "texture" in used:
        for dimension in (2, 3):
            sampler, vector = f"sampler{dimension}D", f"float{dimension}"
            result.append(f"float4 bcglslTexture{dimension}D({sampler} s,{vector} uv){{return tex{dimension}D(s,uv);}}")
            coordinate = "float4(uv,0.0,lod)" if dimension == 2 else "float4(uv,lod)"
            for mode in ("Lod", "Bias"):
                result.append(f"float4 bcglslTexture{dimension}D{mode}({sampler} s,{vector} uv,float lod){{return tex{dimension}D{mode.lower()}(s,{coordinate});}}")
            result.append(f"float4 bcglslTexture{dimension}DGrad({sampler} s,{vector} uv,{vector} dx,{vector} dy){{return tex{dimension}Dgrad(s,uv,dx,dy);}}")
    return "\n".join(result)


def convert_shader(source):
    return Shader(source).convert()
