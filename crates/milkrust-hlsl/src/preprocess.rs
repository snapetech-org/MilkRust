//! Token-based, bounded shader preprocessing. File inclusion is deliberately
//! unavailable: preset text cannot read files or fetch network resources.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_TOKENS: usize = 65536;
const MAX_EXPANSIONS: usize = 16384;
#[derive(Clone)]
struct Macro { parameters: Option<Vec<String>>, body: Vec<String> }
#[derive(Clone, Copy)]
struct Branch { parent: bool, taken: bool, active: bool, saw_else: bool }

pub fn tokens(source: &str) -> Result<Vec<String>, String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() { i += 1; continue; }
        if !bytes[i].is_ascii() || bytes[i] == 0 { return Err("Non-ASCII HLSL token".into()); }
        let start = i;
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
        } else if bytes[i].is_ascii_digit() || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.'
                || ((bytes[i] == b'+' || bytes[i] == b'-') && matches!(bytes[i - 1], b'e' | b'E'))) { i += 1; }
        } else if matches!(bytes[i], b'\'' | b'"') {
            let quote = bytes[i]; i += 1;
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' { i += 1; }
                i += 1;
            }
            if i >= bytes.len() { return Err("Unterminated preprocessor string".into()); }
            i += 1;
        } else {
            i += 1;
            if bytes[start..].starts_with(b"<<=") || bytes[start..].starts_with(b">>=") { i += 2; }
            else if bytes.get(start..start + 2).is_some_and(|pair|
                [b"##", b"&&", b"||", b"<<", b">>", b"<=", b">=", b"==", b"!=", b"++", b"--", b"+=", b"-=", b"*=", b"/=", b"%=", b"&=", b"|=", b"^="]
                    .iter().any(|op| pair == op.as_slice())) { i += 1; }
        }
        if i - start > 1023 { return Err("HLSL token length limit exceeded".into()); }
        out.push(source[start..i].to_string());
        if out.len() > MAX_TOKENS { return Err("HLSL token limit exceeded".into()); }
    }
    Ok(out)
}

fn uncomment(source: &str) -> Result<String, String> {
    let spliced = source.replace("\\\r\n", "").replace("\\\n", "");
    let mut out = String::new();
    let mut chars = spliced.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for c in chars.by_ref() { if c == '\n' { out.push('\n'); break; } }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next(); out.push(' ');
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') { chars.next(); closed = true; break; }
                if c == '\n' { out.push('\n'); }
            }
            if !closed { return Err("Unterminated HLSL comment".into()); }
        } else if c == '"' || c == '\'' {
            out.push(c);
            let mut closed = false;
            while let Some(next) = chars.next() {
                out.push(next);
                if next == '\\' { if let Some(escaped) = chars.next() { out.push(escaped); } }
                else if next == c { closed = true; break; }
            }
            if !closed { return Err("Unterminated HLSL string".into()); }
        } else { out.push(c); }
    }
    Ok(out)
}

fn identifier(s: &str) -> bool {
    s.as_bytes().first().is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

// Keep the suppression set on each token while rescanning replacement tokens
// together with the remaining input. A global disabled set loses the boundary
// between an object-like alias and a following function invocation.
#[derive(Clone)]
struct ExpansionToken { text: String, hidden: [u64; 8], depth: u8 }
impl ExpansionToken {
    fn plain(text: String) -> Self { Self { text, hidden: [0; 8], depth: 0 } }
    fn suppress(&mut self, hidden: &[u64; 8], depth: u8) {
        for (value, added) in self.hidden.iter_mut().zip(hidden) { *value |= added; }
        self.depth = self.depth.max(depth);
    }
}

fn expand_stream(input: Vec<ExpansionToken>, macros: &BTreeMap<String, Macro>,
    ids: &BTreeMap<&str, usize>, nesting: u8, budget: &mut usize) -> Result<Vec<ExpansionToken>, String> {
    if nesting > 64 { return Err("Macro expansion depth exceeded".into()); }
    let mut queue: VecDeque<_> = input.into();
    let mut out = Vec::new();
    while let Some(token) = queue.pop_front() {
        let Some((&id, definition)) = ids.get(token.text.as_str()).zip(macros.get(&token.text)) else {
            out.push(token); continue;
        };
        if token.hidden[id / 64] & (1u64 << (id % 64)) != 0 ||
            (definition.parameters.is_some() && queue.front().map(|t| t.text.as_str()) != Some("(")) {
            out.push(token); continue;
        }
        let depth = token.depth.max(nesting).checked_add(1).ok_or("Macro expansion depth exceeded")?;
        if depth > 64 { return Err("Macro expansion depth exceeded".into()); }
        *budget += 1;
        if *budget > MAX_EXPANSIONS { return Err("Macro expansion budget exceeded".into()); }
        let mut hidden = token.hidden;
        let mut arguments = Vec::new();
        if let Some(parameters) = &definition.parameters {
            queue.pop_front(); // opening parenthesis
            arguments.push(Vec::new());
            let mut level = 1usize;
            loop {
                let next = queue.pop_front().ok_or("Unclosed macro invocation")?;
                if next.text == ")" {
                    level -= 1;
                    if level == 0 {
                        // Function-like expansion hides only names suppressed
                        // at both ends of the invocation, then its own name.
                        for (value, closing) in hidden.iter_mut().zip(next.hidden) { *value &= closing; }
                        break;
                    }
                } else if next.text == "(" {
                    level += 1;
                    if level > 128 { return Err("Macro argument nesting exceeded".into()); }
                }
                if next.text == "," && level == 1 { arguments.push(Vec::new()); }
                else { arguments.last_mut().expect("argument exists").push(next); }
            }
            if parameters.is_empty() && arguments.len() == 1 && arguments[0].is_empty() { arguments.clear(); }
            if arguments.len() != parameters.len() { return Err(format!("Wrong argument count for macro {}", token.text)); }
        }
        hidden[id / 64] |= 1u64 << (id % 64);
        let mut expanded_arguments: Vec<Option<Vec<ExpansionToken>>> = vec![None; arguments.len()];
        let mut replacement = Vec::new();
        for (j, body_token) in definition.body.iter().enumerate() {
            if body_token == "#" { return Err("Stringifying shader macros is unsupported".into()); }
            let parameter = definition.parameters.as_ref().and_then(|p| p.iter().position(|p| p == body_token));
            if let Some(index) = parameter {
                let paste = j > 0 && definition.body[j - 1] == "##" || definition.body.get(j + 1).map(String::as_str) == Some("##");
                if !paste && expanded_arguments[index].is_none() {
                    expanded_arguments[index] = Some(expand_stream(arguments[index].clone(), macros, ids, nesting + 1, budget)?);
                }
                let values = if paste { &arguments[index] } else { expanded_arguments[index].as_ref().expect("expanded argument") };
                if paste && values.is_empty() { replacement.push(ExpansionToken::plain(String::new())); }
                else { replacement.extend(values.iter().cloned()); }
            } else { replacement.push(ExpansionToken::plain(body_token.clone())); }
            if replacement.len() > MAX_TOKENS { return Err("Macro replacement token limit exceeded".into()); }
        }
        let mut pasted: Vec<ExpansionToken> = Vec::new();
        let mut replacement = replacement.into_iter();
        while let Some(next) = replacement.next() {
            if next.text == "##" {
                let mut left = pasted.pop().ok_or("Missing left token for ##")?;
                let right = replacement.next().ok_or("Missing right token for ##")?;
                left.text.push_str(&right.text);
                if !left.text.is_empty() && tokens(&left.text)?.len() != 1 { return Err("Invalid pasted token".into()); }
                for (value, right) in left.hidden.iter_mut().zip(right.hidden) { *value &= right; }
                left.depth = left.depth.max(right.depth);
                pasted.push(left);
            } else { pasted.push(next); }
        }
        pasted.retain(|t| !t.text.is_empty()); // Empty pasted arguments are placemarkers.
        if out.len() + queue.len() + pasted.len() > MAX_TOKENS { return Err("Macro output limit exceeded".into()); }
        for mut next in pasted.into_iter().rev() { next.suppress(&hidden, depth); queue.push_front(next); }
    }
    Ok(out)
}

fn expand(input: &[String], macros: &BTreeMap<String, Macro>, disabled: &mut BTreeSet<String>,
    depth: usize, budget: &mut usize) -> Result<Vec<String>, String> {
    if depth > 64 { return Err("Macro expansion depth exceeded".into()); }
    let ids: BTreeMap<&str, usize> = macros.keys().enumerate().map(|(i, name)| (name.as_str(), i)).collect();
    let mut hidden = [0u64; 8];
    for name in disabled.iter() { if let Some(&id) = ids.get(name.as_str()) { hidden[id / 64] |= 1u64 << (id % 64); } }
    let input = input.iter().map(|text| ExpansionToken { text: text.clone(), hidden, depth: depth as u8 }).collect();
    Ok(expand_stream(input, macros, &ids, depth as u8, budget)?.into_iter().map(|t| t.text).collect())
}

fn condition(input: &[String], macros: &BTreeMap<String, Macro>, budget: &mut usize) -> Result<bool, String> {
    let mut defined = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i] == "defined" {
            i += 1;
            let paren = input.get(i).map(String::as_str) == Some("(");
            if paren { i += 1; }
            let name = input.get(i).filter(|s| identifier(s)).ok_or("Expected name after defined")?;
            defined.push(if macros.contains_key(name) { "1" } else { "0" }.into());
            i += 1;
            if paren { if input.get(i).map(String::as_str) != Some(")") { return Err("Unclosed defined()".into()); } i += 1; }
        } else { defined.push(input[i].clone()); i += 1; }
    }
    let mut expanded = expand(&defined, macros, &mut BTreeSet::new(), 0, budget)?;
    for t in &mut expanded { if identifier(t) { *t = "0".into(); } }
    let mut parser = IntegerExpression { tokens: &expanded, cursor: 0, depth: 0 };
    let value = parser.expression(0, true)?;
    if parser.cursor != expanded.len() { return Err("Invalid #if expression".into()); }
    Ok(value != 0)
}

struct IntegerExpression<'a> { tokens: &'a [String], cursor: usize, depth: usize }
impl IntegerExpression<'_> {
    fn expression(&mut self, min: u8, evaluate: bool) -> Result<i64, String> {
        self.depth += 1;
        if self.depth > 128 { return Err("#if nesting limit exceeded".into()); }
        let token = self.tokens.get(self.cursor).ok_or("Missing #if operand")?.as_str();
        self.cursor += 1;
        let mut left = match token {
            "!" => i64::from(self.expression(12, evaluate)? == 0),
            "~" => !self.expression(12, evaluate)?,
            "+" => self.expression(12, evaluate)?,
            "-" => self.expression(12, evaluate)?.wrapping_neg(),
            "(" => {
                let value = self.expression(0, evaluate)?;
                if self.tokens.get(self.cursor).map(String::as_str) != Some(")") { return Err("Unclosed #if expression".into()); }
                self.cursor += 1; value
            },
            _ => {
                let number = token.trim_end_matches(['u', 'U', 'l', 'L']);
                let (number, radix) = if let Some(n) = number.strip_prefix("0x").or_else(|| number.strip_prefix("0X")) { (n, 16) }
                    else if number.len() > 1 && number.starts_with('0') { (&number[1..], 8) } else { (number, 10) };
                u64::from_str_radix(number, radix).map_err(|_| "Invalid #if integer")? as i64
            }
        };
        while let Some(op) = self.tokens.get(self.cursor).map(String::as_str) {
            let precedence = match op { "||" => 1, "&&" => 2, "|" => 3, "^" => 4, "&" => 5,
                "==" | "!=" => 6, "<" | "<=" | ">" | ">=" => 7, "<<" | ">>" => 8,
                "+" | "-" => 9, "*" | "/" | "%" => 10, _ => break };
            if precedence < min { break; }
            self.cursor += 1;
            let rhs_evaluate = evaluate && !(op == "&&" && left == 0 || op == "||" && left != 0);
            let right = self.expression(precedence + 1, rhs_evaluate)?;
            left = match op {
                "||" => i64::from(left != 0 || right != 0), "&&" => i64::from(left != 0 && right != 0),
                "|" => left | right, "^" => left ^ right, "&" => left & right,
                "==" => i64::from(left == right), "!=" => i64::from(left != right), "<" => i64::from(left < right),
                "<=" => i64::from(left <= right), ">" => i64::from(left > right), ">=" => i64::from(left >= right),
                "+" => left.wrapping_add(right), "-" => left.wrapping_sub(right), "*" => left.wrapping_mul(right),
                "/" | "%" if right == 0 && !rhs_evaluate => 0,
                "/" | "%" if right == 0 => return Err("Division by zero in #if".into()),
                "/" => left.wrapping_div(right), "%" => left.wrapping_rem(right),
                "<<" | ">>" if !(0..64).contains(&right) && !rhs_evaluate => 0,
                "<<" | ">>" if !(0..64).contains(&right) => return Err("Invalid #if shift".into()),
                "<<" => left.wrapping_shl(right as u32), ">>" => left.wrapping_shr(right as u32), _ => unreachable!()
            };
        }
        if min == 0 && self.tokens.get(self.cursor).map(String::as_str) == Some("?") {
            self.cursor += 1;
            let yes = self.expression(0, evaluate && left != 0)?;
            if self.tokens.get(self.cursor).map(String::as_str) != Some(":") { return Err("Missing #if ternary colon".into()); }
            self.cursor += 1;
            let no = self.expression(0, evaluate && left == 0)?;
            left = if left != 0 { yes } else { no };
        }
        self.depth -= 1;
        Ok(left)
    }
}

pub fn preprocess(source: &str) -> Result<String, String> {
    if source.len() > super::MAX_SOURCE { return Err("HLSL source limit exceeded".into()); }
    let clean = uncomment(source)?;
    let mut macros: BTreeMap<String, Macro> = BTreeMap::new();
    let mut branches: Vec<Branch> = Vec::new();
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let mut budget = 0;
    for (line_number, line) in clean.lines().enumerate() {
        let line = line.trim();
        let active = branches.last().map_or(true, |b| b.active);
        if let Some(directive) = line.strip_prefix('#') {
            result.extend(expand(&pending, &macros, &mut BTreeSet::new(), 0, &mut budget)?);
            pending.clear();
            let directive = directive.trim_start();
            let end = directive.find(char::is_whitespace).unwrap_or(directive.len());
            let (name, argument) = directive.split_at(end);
            let argument = argument.trim_start();
            let t = tokens(argument)?;
            match name {
                "if" | "ifdef" | "ifndef" => {
                    if branches.len() >= 64 { return Err("Preprocessor conditional nesting exceeded".into()); }
                    let test = if name == "if" { active && condition(&t, &macros, &mut budget)? }
                        else { if t.len() != 1 || !identifier(&t[0]) { return Err("Invalid conditional macro name".into()); }
                            active && (macros.contains_key(&t[0]) == (name == "ifdef")) };
                    branches.push(Branch { parent: active, active: test, taken: test, saw_else: false });
                }
                "elif" => {
                    let b = branches.last_mut().ok_or("#elif without #if")?;
                    if b.saw_else { return Err("#elif after #else".into()); }
                    b.active = b.parent && !b.taken && condition(&t, &macros, &mut budget)?;
                    b.taken |= b.active;
                }
                "else" => {
                    let b = branches.last_mut().ok_or("#else without #if")?;
                    if b.saw_else || !t.is_empty() { return Err("Invalid #else".into()); }
                    b.active = b.parent && !b.taken; b.taken = true; b.saw_else = true;
                }
                "endif" => { if branches.pop().is_none() || !t.is_empty() { return Err("Invalid #endif".into()); } }
                _ if !active => {},
                "define" => {
                    let macro_name = t.first().filter(|t| identifier(t)).ok_or("Invalid macro name")?.clone();
                    let function = argument[macro_name.len()..].starts_with('(');
                    let mut start = 1;
                    let parameters = if function {
                        start = 2;
                        let mut p = Vec::new();
                        while t.get(start).map(String::as_str) != Some(")") {
                            let name = t.get(start).filter(|s| identifier(s)).ok_or("Invalid macro parameter")?.clone();
                            if p.contains(&name) || p.len() >= 32 { return Err("Duplicate or excess macro parameters".into()); }
                            p.push(name); start += 1;
                            if t.get(start).map(String::as_str) == Some(")") { break; }
                            if t.get(start).map(String::as_str) != Some(",") { return Err("Invalid macro parameter list".into()); }
                            start += 1;
                        }
                        start += 1; Some(p)
                    } else { None };
                    if macros.len() >= 512 && !macros.contains_key(&macro_name) { return Err("Macro count limit exceeded".into()); }
                    macros.insert(macro_name, Macro { parameters, body: t[start..].to_vec() });
                }
                "undef" => { if t.len() != 1 { return Err("Invalid #undef".into()); } macros.remove(&t[0]); }
                "error" => return Err(format!("Shader #error on line {}: {}", line_number + 1, argument)),
                "line" => {}, // Source cannot alter file access or the bounded diagnostic sink.
                "pragma" if t.as_slice() == ["once"] ||
                    (t.len() == 4 && t[0] == "pack_matrix" && t[1] == "(" && t[3] == ")" && matches!(t[2].as_str(), "row_major" | "column_major")) => {
                    // Includes are disabled and host matrices use logical values,
                    // not user-controlled constant-buffer packing.
                },
                "" => {},
                _ => return Err(format!("Unsupported shader directive #{name}")),
            }
        } else if active { pending.extend(tokens(line)?); }
        if result.len() + pending.len() > MAX_TOKENS { return Err("Preprocessor output token limit exceeded".into()); }
    }
    if !branches.is_empty() { return Err("Unterminated shader conditional".into()); }
    result.extend(expand(&pending, &macros, &mut BTreeSet::new(), 0, &mut budget)?);
    if result.len() > MAX_TOKENS { return Err("Preprocessor output token limit exceeded".into()); }
    let result = result.join(" ");
    if result.len() > super::MAX_SOURCE { return Err("Preprocessor output byte limit exceeded".into()); }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compound_shifts_keep_their_token_identity() {
        assert_eq!(tokens("x<<=2;y>>=1;").unwrap(), ["x", "<<=", "2", ";", "y", ">>=", "1", ";"]);
    }
    #[test]
    fn rescan_follows_aliases_without_reexpanding_recursive_tokens() {
        assert_eq!(preprocess("#define ALIAS SCALE\n#define SCALE(x) ((x)*2)\nALIAS(3)").unwrap(), "( ( 3 ) * 2 )");
        assert_eq!(preprocess("#define PICK() SCALE\n#define SCALE(x) (x+1)\nPICK()(4)").unwrap(), "( 4 + 1 )");
        assert_eq!(preprocess("#define F(x) x\nF(F(1))").unwrap(), "1");
        assert_eq!(preprocess("#define A B\n#define B A\nA").unwrap(), "A");
        assert_eq!(preprocess("#define F(x) x F\nF(1)(2)").unwrap(), "1 F ( 2 )");
    }
    #[test]
    fn empty_paste_arguments_are_placemarkers() {
        assert_eq!(preprocess("#define CAT(a,b) a ## b\nCAT(,uv) CAT(uv,) CAT(,) ").unwrap(), "uv uv");
        assert_eq!(preprocess("#define UNUSED(x) 2\n#define BAD(x) x\nUNUSED(BAD(1,2))").unwrap(), "2");
    }
    #[test]
    fn non_ascii_adjacent_to_an_operator_returns_a_diagnostic() {
        for source in ["+é", "&é", "<<é", "!=水"] { assert!(tokens(source).is_err()); }
        assert_eq!(preprocess("#pragma pack_matrix(row_major)\nret=1;").unwrap(), "ret = 1 ;");
        assert!(preprocess("#pragma unknown_option\nret=1;").is_err());
    }
    #[test]
    fn macros_conditions_and_paste() {
        assert_eq!(preprocess("#define Q(n) q ## n\n#define SCALE(x) ((x)*2)\n#if defined(Q) && (4 << 1) == 8\nret = SCALE(Q(1));\n#else\n#error inactive\n#endif").unwrap(), "ret = ( ( q1 ) * 2 ) ;");
    }
    #[test]
    fn conditions_short_circuit_and_multiline_calls() {
        assert_eq!(preprocess("#define F(x,y) x+y\n#if 0 && 1/0\n#error no\n#endif\nF(1,\n2)").unwrap(), "1 + 2");
    }
    #[test]
    fn failed_directives_and_expansion_are_reported() {
        assert!(preprocess("#include \"private.h\"").is_err());
        assert!(preprocess("#if 1\nret=0;").is_err());
        assert!(preprocess("#define A(x) x\nA(1,2)").is_err());
        assert!(preprocess("#if 1/0\n#endif").is_err());
    }
}
