//! Token-based, bounded shader preprocessing. File inclusion is deliberately
//! unavailable: preset text cannot read files or fetch network resources.
use std::collections::{BTreeMap, BTreeSet};

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
            if i < bytes.len() && ["##", "&&", "||", "<<", ">>", "<=", ">=", "==", "!=", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^="].contains(&&source[start..i + 1]) { i += 1; }
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

fn expand(input: &[String], macros: &BTreeMap<String, Macro>, disabled: &mut BTreeSet<String>,
    depth: usize, budget: &mut usize) -> Result<Vec<String>, String> {
    if depth > 64 { return Err("Macro expansion depth exceeded".into()); }
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let name = &input[i];
        let Some(definition) = macros.get(name).filter(|_| !disabled.contains(name)) else {
            out.push(name.clone()); i += 1;
            if out.len() > MAX_TOKENS { return Err("Macro output limit exceeded".into()); }
            continue;
        };
        let mut replacement = definition.body.clone();
        if let Some(parameters) = &definition.parameters {
            if input.get(i + 1).map(String::as_str) != Some("(") { out.push(name.clone()); i += 1; continue; }
            let mut arguments = vec![Vec::new()];
            let mut level = 1usize;
            i += 2;
            while i < input.len() {
                let t = &input[i];
                if t == ")" { level -= 1; if level == 0 { break; } }
                if t == "(" { level += 1; if level > 128 { return Err("Macro argument nesting exceeded".into()); } }
                if t == "," && level == 1 { arguments.push(Vec::new()); }
                else { arguments.last_mut().expect("argument exists").push(t.clone()); }
                i += 1;
            }
            if level != 0 { return Err("Unclosed macro invocation".into()); }
            if parameters.is_empty() && arguments.len() == 1 && arguments[0].is_empty() { arguments.clear(); }
            if arguments.len() != parameters.len() { return Err(format!("Wrong argument count for macro {name}")); }
            let expanded = arguments.iter().map(|a| expand(a, macros, disabled, depth + 1, budget)).collect::<Result<Vec<_>, _>>()?;
            replacement.clear();
            for (j, t) in definition.body.iter().enumerate() {
                if let Some(index) = parameters.iter().position(|p| p == t) {
                    let paste = j > 0 && definition.body[j - 1] == "##" || definition.body.get(j + 1).map(String::as_str) == Some("##");
                    replacement.extend(if paste { &arguments[index] } else { &expanded[index] }.iter().cloned());
                } else { replacement.push(t.clone()); }
            }
        }
        i += 1;
        *budget += 1;
        if *budget > MAX_EXPANSIONS { return Err("Macro expansion budget exceeded".into()); }
        let mut pasted: Vec<String> = Vec::new();
        let mut j = 0;
        while j < replacement.len() {
            if replacement[j] == "##" {
                let left = pasted.pop().ok_or("Missing left token for ##")?;
                j += 1;
                let right = replacement.get(j).ok_or("Missing right token for ##")?;
                let joined = format!("{left}{right}");
                if tokens(&joined)?.len() != 1 { return Err("Invalid pasted token".into()); }
                pasted.push(joined);
            } else if replacement[j] == "#" { return Err("Stringifying shader macros is unsupported".into()); }
            else { pasted.push(replacement[j].clone()); }
            j += 1;
        }
        disabled.insert(name.clone());
        out.extend(expand(&pasted, macros, disabled, depth + 1, budget)?);
        disabled.remove(name);
        if out.len() > MAX_TOKENS { return Err("Macro output limit exceeded".into()); }
    }
    Ok(out)
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
