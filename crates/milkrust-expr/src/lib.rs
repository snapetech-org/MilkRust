// =============================================================================
// milkrust-expr: Expression tokenizer, recursive-descent parser, and equation
// evaluation engine for MilkDrop-compatible preset scripts.
//
// This crate implements the full expression grammar including:
//   - Tokenization (identifiers, numbers, operators, parentheses)
//   - Recursive-descent parser with precedence climbing
//   - 50+ scoped math functions (sin, cos, atan2, pow, etc.)
//   - Loop/while/exec2/exec3 control flow
//   - megabuf/gmegabuf indexed memory
//   - Deterministic pseudo-random via seeded counter
//
// # Examples
//
// ```
// use std::collections::BTreeMap;
// use milkrust_expr::{evaluate_milkrust_expression, MilkRustValue};
//
// let scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
// let result = evaluate_milkrust_expression("sin(time) + 0.5", &scope).unwrap();
// assert!((result - 1.0).abs() < 0.0001);
// ```
// =============================================================================

use std::collections::BTreeMap;
use std::fmt;

/// Error returned by MilkRust expression evaluation.
#[derive(Clone, Debug, PartialEq)]
pub enum MilkRustError {
    /// An error occurred during tokenization.
    Tokenize {
        /// The expression that failed to tokenize.
        expression: String,
        /// The approximate position in the expression string.
        position: usize,
        /// Human-readable description of the error.
        message: String,
    },
    /// An error occurred during parsing or evaluation.
    Parse {
        /// The expression that failed to parse.
        expression: String,
        /// Human-readable description of the error.
        message: String,
    },
    /// An error occurred during function evaluation.
    Evaluate {
        /// The name of the function that errored.
        function: String,
        /// Human-readable description of the error.
        message: String,
    },
}

impl fmt::Display for MilkRustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MilkRustError::Tokenize {
                expression,
                position,
                message,
            } => write!(
                f,
                "Tokenization error at position {} in '{}': {}",
                position, expression, message
            ),
            MilkRustError::Parse { expression, message } => {
                write!(f, "Parse error in '{}': {}", expression, message)
            }
            MilkRustError::Evaluate { function, message } => {
                write!(f, "Evaluation error in function '{}': {}", function, message)
            }
        }
    }
}

impl std::error::Error for MilkRustError {}

/// A value in the MilkRust expression language.
#[derive(Clone, Debug, PartialEq)]
pub enum MilkRustValue {
    /// A numeric (floating-point) value.
    Number(f64),
    /// A text (string) value.
    Text(String),
}

impl MilkRustValue {
    /// Returns the numeric value if this is a `Number`, or `None` otherwise.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(_) => None,
        }
    }

    /// Returns the text representation of this value.
    ///
    /// For `Number` values, integers are rendered without a decimal point
    /// (e.g. `42` not `42.0`), while non-integers retain their decimal form.
    /// `Text` values are returned as-is.
    pub fn as_text(&self) -> String {
        match self {
            Self::Number(value) => {
                if value.fract().abs() < f64::EPSILON {
                    format!("{}", *value as i64)
                } else {
                    format!("{value}")
                }
            }
            Self::Text(value) => value.clone(),
        }
    }

    /// Returns `true` if this is a `Number` variant.
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    /// Returns `true` if this is a `Text` variant.
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Returns the string value if this is `Text`, or `None` otherwise.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// Returns `true` if the value is truthy in a MilkDrop-style boolean context.
    ///
    /// A `Number` is truthy if it is non-zero. A `Text` is truthy if it is non-empty.
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Number(value) => *value != 0.0,
            Self::Text(value) => !value.is_empty(),
        }
    }
}

impl From<f64> for MilkRustValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<&str> for MilkRustValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for MilkRustValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for MilkRustValue {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<usize> for MilkRustValue {
    fn from(value: usize) -> Self {
        Self::Number(value as f64)
    }
}

impl fmt::Display for MilkRustToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident(name) => write!(f, "Ident({name})"),
            Self::Number(value) => write!(f, "Number({value})"),
            Self::Op(op) => write!(f, "Op({op})"),
        }
    }
}

impl fmt::Display for MilkRustValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => {
                if value.fract().abs() < f64::EPSILON {
                    write!(f, "{}", *value as i64)
                } else {
                    write!(f, "{value}")
                }
            }
            Self::Text(value) => write!(f, "{value}"),
        }
    }
}

/// A token in the MilkRust expression language.
#[derive(Clone, Debug, PartialEq)]
pub enum MilkRustToken {
    /// An identifier (variable or function name).
    Ident(String),
    /// A numeric literal.
    Number(f64),
    /// An operator or delimiter.
    Op(String),
}

/// Strip `//` comments from an equation block.
///
/// Removes everything after `//` on each line, which is how MilkDrop
/// preset equations embed comments. This is useful for any tool that
/// needs to preprocess MilkRust equation text before parsing.
///
/// # Examples
///
/// ```
/// use milkrust_expr::strip_milkrust_equation_comments;
///
/// let cleaned = strip_milkrust_equation_comments("a = 1; // this is a comment\nb = 2");
/// assert_eq!(cleaned, "a = 1; \nb = 2");
/// ```
pub fn strip_milkrust_equation_comments(text: &str) -> String {
    text.lines()
        .map(|line| line.split_once("//").map(|(code, _)| code).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Tokenize a MilkDrop expression string into a sequence of tokens.
///
/// Returns an error if the expression contains unsupported characters.
///
/// # Examples
///
/// ```
/// use milkrust_expr::{tokenize_milkrust_expression, MilkRustToken};
///
/// let tokens = tokenize_milkrust_expression("time + 0.5").unwrap();
/// assert_eq!(tokens.len(), 3);
/// ```
pub fn tokenize_milkrust_expression(expression: &str) -> Result<Vec<MilkRustToken>, MilkRustError> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric()
                    || chars[index] == '_'
                    || chars[index] == '.')
            {
                index += 1;
            }
            tokens.push(MilkRustToken::Ident(
                chars[start..index]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            ));
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
                index += 1;
            }
            if index < chars.len() && matches!(chars[index], 'e' | 'E') {
                index += 1;
                if index < chars.len() && matches!(chars[index], '+' | '-') {
                    index += 1;
                }
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
            }
            let value = chars[start..index]
                .iter()
                .collect::<String>()
                .parse::<f64>()
                .map_err(|_| {
                    MilkRustError::Tokenize {
                        expression: expression.to_string(),
                        position: start,
                        message: "Invalid numeric literal".to_string(),
                    }
                })?;
            tokens.push(MilkRustToken::Number(value));
            continue;
        }
        let two = if index + 1 < chars.len() {
            Some([chars[index], chars[index + 1]].iter().collect::<String>())
        } else {
            None
        };
        if let Some(two) = two.as_deref().filter(|value| {
            matches!(
                *value,
                "&&" | "||" | "<<" | ">>" | "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|="
            )
        }) {
            tokens.push(MilkRustToken::Op(two.to_string()));
            index += 2;
            continue;
        }
        if matches!(
            ch,
            '(' | ')'
                | '+'
                | '-'
                | '*'
                | '/'
                | '%'
                | ','
                | ';'
                | '?'
                | ':'
                | '<'
                | '>'
                | '&'
                | '|'
                | '^'
                | '='
                | '!'
                | '~'
        ) {
            tokens.push(MilkRustToken::Op(ch.to_string()));
            index += 1;
            continue;
        }
        return Err(MilkRustError::Tokenize {
            expression: expression.to_string(),
            position: index,
            message: format!("Unsupported character: '{}'", ch),
        });
    }
    Ok(tokens)
}

fn milkrust_truthy(value: f64) -> bool {
    value.abs() > 0.00001
}

fn milkrust_pow(base: f64, exponent: f64) -> f64 {
    if base.abs() < 0.00001 && exponent < 0.0 {
        return 0.0;
    }
    let value = base.powf(exponent);
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn milkrust_number(scope: &BTreeMap<String, MilkRustValue>, name: &str) -> f64 {
    scope
        .get(name)
        .and_then(MilkRustValue::as_number)
        .unwrap_or(0.0)
}

fn milkrust_buffer_key(name: &str, index: f64) -> String {
    let prefix = if name.eq_ignore_ascii_case("gmegabuf") {
        "gmegabuf"
    } else {
        "megabuf"
    };
    let index = if index.is_finite() {
        index.trunc().max(0.0) as usize
    } else {
        0
    };
    format!("{prefix}_{index}")
}

fn milkrust_buffer_number(
    scope: &BTreeMap<String, MilkRustValue>,
    name: &str,
    index: f64,
) -> f64 {
    milkrust_number(scope, &milkrust_buffer_key(name, index))
}

fn milkrust_indexed_sample(values: &[f64], position: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let normalized = position.clamp(0.0, 1.0);
    let index = ((normalized * values.len() as f64).floor() as usize).min(values.len() - 1);
    let value = values[index];
    if value > 1.0 {
        value / 255.0
    } else {
        value
    }
}

fn mix_milkrust_rand_seed(mut seed: u64, value: f64) -> u64 {
    seed ^= value.to_bits().wrapping_add(0x9e37_79b9_7f4a_7c15);
    seed = seed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    seed ^ (seed >> 31)
}

fn milkrust_pseudo_random_unit(scope: &BTreeMap<String, MilkRustValue>, counter: usize) -> f64 {
    let mut seed = 0x4d49_4c4b_4452_4f50u64 ^ counter as u64;
    for key in [
        "time", "frame", "bass", "mid", "treb", "bass_att", "mid_att", "treb_att",
    ] {
        seed = mix_milkrust_rand_seed(seed, milkrust_number(scope, key));
    }
    seed = mix_milkrust_rand_seed(seed, counter as f64 + 0.123_456_789);
    seed ^= seed >> 12;
    seed ^= seed << 25;
    seed ^= seed >> 27;
    let value = seed.wrapping_mul(0x2545_f491_4f6c_dd1d);
    (value as f64) / (u64::MAX as f64)
}

const RUSTYMILK_MAX_LOOP_ITERATIONS: usize = 200_000;
const MILKRUST_MEMORY_LENGTH: usize = 8_388_608;

struct MilkRustExpressionParser<'a> {
    rand_counter: usize,
    scope: BTreeMap<String, MilkRustValue>,
    tokens: &'a [MilkRustToken],
    index: usize,
    expression: &'a str,
    evaluate: bool,
}

impl<'a> MilkRustExpressionParser<'a> {
    fn new(
        tokens: &'a [MilkRustToken],
        scope: BTreeMap<String, MilkRustValue>,
        rand_counter: usize,
        expression: &'a str,
        evaluate: bool,
    ) -> Self {
        Self {
            rand_counter,
            scope,
            tokens,
            index: 0,
            expression,
            evaluate,
        }
    }

    fn peek_op(&self) -> Option<&str> {
        match self.tokens.get(self.index) {
            Some(MilkRustToken::Op(value)) => Some(value),
            _ => None,
        }
    }

    fn remaining_call_args(&mut self, name: &str) -> Result<Vec<&'a [MilkRustToken]>, String> {
        let mut args: Vec<&[MilkRustToken]> = Vec::new();
        let mut depth = 0usize;
        let mut arg_start = self.index;

        while self.index < self.tokens.len() {
            let token = &self.tokens[self.index];
            self.index += 1;

            match token {
                MilkRustToken::Op(value) if value == "(" => {
                    depth += 1;
                }
                MilkRustToken::Op(value) if value == ")" => {
                    if depth == 0 {
                        if arg_start + 1 < self.index {
                            args.push(&self.tokens[arg_start..self.index - 1]);
                        }
                        return Ok(args);
                    }
                    depth -= 1;
                }
                MilkRustToken::Op(value) if (value == "," || value == ";") && depth == 0 => {
                    args.push(&self.tokens[arg_start..self.index - 1]);
                    arg_start = self.index;
                }
                _ => {}
            }
        }
        Err(format!("Unclosed function call: {name}"))
    }

    fn evaluate_arg_tokens(&mut self, tokens: &[MilkRustToken]) -> Result<f64, String> {
        if !self.evaluate {
            let mut parser = MilkRustExpressionParser::new(
                tokens,
                self.scope.clone(),
                self.rand_counter,
                self.expression,
                false,
            );
            parser.parse()?;
            return Ok(0.0);
        }
        if let Some((target_tokens, operator, value_tokens)) = self.assignment_arg_parts(tokens) {
            let Some(key) = self.lvalue_key(target_tokens)? else {
                return Err(format!("{operator} requires a variable or buffer target."));
            };
            let current = milkrust_number(&self.scope, &key);
            let next = self.evaluate_arg_tokens(value_tokens)?;
            let value = apply_milkrust_assignment_operator(current, operator, next);
            self.scope.insert(key, MilkRustValue::Number(value));
            return Ok(value);
        }
        let mut parser = MilkRustExpressionParser::new(
            tokens,
            self.scope.clone(),
            self.rand_counter,
            self.expression,
            self.evaluate,
        );
        let value = parser.parse()?;
        self.scope = parser.scope;
        self.rand_counter = parser.rand_counter;
        Ok(value)
    }

    fn assignment_arg_parts<'b>(
        &self,
        tokens: &'b [MilkRustToken],
    ) -> Option<(&'b [MilkRustToken], &'b str, &'b [MilkRustToken])> {
        let mut depth = 0usize;
        for (index, token) in tokens.iter().enumerate() {
            match token {
                MilkRustToken::Op(value) if value == "(" => depth += 1,
                MilkRustToken::Op(value) if value == ")" => depth = depth.saturating_sub(1),
                MilkRustToken::Op(value)
                    if depth == 0 && matches!(value.as_str(), "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|=") =>
                {
                    return Some((&tokens[..index], value, &tokens[index + 1..]));
                }
                _ => {}
            }
        }
        None
    }

    fn assign_arg_tokens(
        &mut self,
        target_tokens: &[MilkRustToken],
        value: f64,
    ) -> Result<f64, String> {
        let Some(key) = self.lvalue_key(target_tokens)? else {
            return Err("assign() requires a variable or buffer target.".to_string());
        };
        self.scope.insert(key, MilkRustValue::Number(value));
        Ok(value)
    }

    fn lvalue_key(&mut self, tokens: &[MilkRustToken]) -> Result<Option<String>, String> {
        match tokens {
            [MilkRustToken::Ident(name)] => Ok(Some(name.to_ascii_lowercase())),
            [MilkRustToken::Ident(name), MilkRustToken::Op(open), rest @ .., MilkRustToken::Op(close)]
                if open == "(" && close == ")" && name.eq_ignore_ascii_case("if") =>
            {
                let args = split_milkrust_call_tokens(rest);
                if args.len() < 3 {
                    return Ok(None);
                }
                let condition = self.evaluate_arg_tokens(args[0])?;
                self.lvalue_key(if milkrust_truthy(condition) { args[1] } else { args[2] })
            }
            [MilkRustToken::Ident(name), MilkRustToken::Op(open), rest @ .., MilkRustToken::Op(close)]
                if open == "("
                    && close == ")"
                    && (name.eq_ignore_ascii_case("megabuf")
                        || name.eq_ignore_ascii_case("gmegabuf")) =>
            {
                let index = self.evaluate_arg_tokens(rest)?;
                Ok(Some(milkrust_buffer_key(name, index)))
            }
            _ => Ok(None),
        }
    }

    fn call_special_function(
        &mut self,
        name: &str,
        args: &[&[MilkRustToken]],
    ) -> Result<Option<f64>, String> {
        match name {
            "assign" => {
                if args.len() < 2 {
                    return Ok(Some(0.0));
                }
                let value = self.evaluate_arg_tokens(args[1])?;
                if let Some(key) = self.lvalue_key(args[0])? {
                    self.scope.insert(key, MilkRustValue::Number(value));
                }
                Ok(Some(value))
            }
            "if" => {
                if args.len() < 3 {
                    return Ok(Some(0.0));
                }
                let condition = self.evaluate_arg_tokens(args[0])?;
                let choose_true = milkrust_truthy(condition);
                let skipped = if choose_true { args[2] } else { args[1] };
                let selected = if choose_true { args[1] } else { args[2] };
                let outer_evaluate = self.evaluate;
                self.evaluate = false;
                let skipped_result = self.evaluate_arg_tokens(skipped);
                self.evaluate = outer_evaluate;
                skipped_result?;
                self.evaluate_arg_tokens(selected).map(Some)
            }
            "exec2" | "exec3" => {
                let mut first = 0.0;
                for (index, arg) in args.iter().enumerate() {
                    let value = self.evaluate_arg_tokens(arg)?;
                    if index == 0 {
                        first = value;
                    }
                }
                Ok(Some(first))
            }
            "loop" => {
                if args.is_empty() {
                    return Ok(Some(0.0));
                }
                let count = self
                    .evaluate_arg_tokens(args[0])?
                    .trunc()
                    .clamp(0.0, RUSTYMILK_MAX_LOOP_ITERATIONS as f64)
                    as usize;
                let mut last = 0.0;
                for _ in 0..count {
                    for arg in &args[1..] {
                        last = self.evaluate_arg_tokens(arg)?;
                    }
                }
                Ok(Some(last))
            }
            "while" => {
                if args.is_empty() {
                    return Ok(Some(0.0));
                }
                let mut last = 0.0;
                for _ in 0..RUSTYMILK_MAX_LOOP_ITERATIONS {
                    let condition = self.evaluate_milkrust_while_condition(args[0])?;
                    if !milkrust_truthy(condition) {
                        break;
                    }
                    last = condition;
                    for arg in &args[1..] {
                        last = self.evaluate_arg_tokens(arg)?;
                    }
                }
                Ok(Some(last))
            }
            "freembuf" => {
                if let Some(index) = args.first() {
                    self.evaluate_arg_tokens(index)?;
                }
                Ok(Some(0.0))
            }
            "memset" => {
                if args.len() < 3 {
                    return Ok(Some(0.0));
                }
                let dest = self.evaluate_arg_tokens(args[0])?
                    .trunc()
                    .clamp(0.0, (MILKRUST_MEMORY_LENGTH - 1) as f64) as usize;
                let value = self.evaluate_arg_tokens(args[1])?;
                let requested = self.evaluate_arg_tokens(args[2])?
                    .trunc()
                    .clamp(0.0, RUSTYMILK_MAX_LOOP_ITERATIONS as f64) as usize;
                let count = requested.min(MILKRUST_MEMORY_LENGTH - dest);
                for offset in 0..count {
                    self.scope.insert(
                        milkrust_buffer_key("megabuf", (dest + offset) as f64),
                        MilkRustValue::Number(value),
                    );
                }
                Ok(Some(milkrust_number(
                    &self.scope,
                    &milkrust_buffer_key("megabuf", dest as f64),
                )))
            }
            "memcpy" => {
                if args.len() < 3 {
                    return Ok(Some(0.0));
                }
                let dest = self.evaluate_arg_tokens(args[0])?
                    .trunc()
                    .clamp(0.0, (MILKRUST_MEMORY_LENGTH - 1) as f64) as usize;
                let source = self.evaluate_arg_tokens(args[1])?
                    .trunc()
                    .clamp(0.0, (MILKRUST_MEMORY_LENGTH - 1) as f64) as usize;
                let requested = self.evaluate_arg_tokens(args[2])?
                    .trunc()
                    .clamp(0.0, RUSTYMILK_MAX_LOOP_ITERATIONS as f64) as usize;
                let count = requested
                    .min(MILKRUST_MEMORY_LENGTH - dest)
                    .min(MILKRUST_MEMORY_LENGTH - source);
                let values = (0..count)
                    .map(|offset| {
                        milkrust_number(
                            &self.scope,
                            &milkrust_buffer_key("megabuf", (source + offset) as f64),
                        )
                    })
                    .collect::<Vec<_>>();
                for (offset, value) in values.into_iter().enumerate() {
                    self.scope.insert(
                        milkrust_buffer_key("megabuf", (dest + offset) as f64),
                        MilkRustValue::Number(value),
                    );
                }
                Ok(Some(milkrust_number(
                    &self.scope,
                    &milkrust_buffer_key("megabuf", dest as f64),
                )))
            }
            _ => Ok(None),
        }
    }

    fn evaluate_milkrust_while_condition(
        &mut self,
        tokens: &[MilkRustToken],
    ) -> Result<f64, String> {
        if let [MilkRustToken::Ident(name), MilkRustToken::Op(open), rest @ .., MilkRustToken::Op(close)] =
            tokens
        {
            if open == "("
                && close == ")"
                && (name.eq_ignore_ascii_case("exec2")
                    || name.eq_ignore_ascii_case("exec3"))
            {
                let args = split_milkrust_call_tokens(rest);
                if args.is_empty() {
                    return Ok(0.0);
                }
                let condition = self.evaluate_arg_tokens(args[0])?;
                if milkrust_truthy(condition) {
                    for arg in &args[1..] {
                        self.evaluate_arg_tokens(arg)?;
                    }
                }
                return Ok(condition);
            }
        }
        self.evaluate_arg_tokens(tokens)
    }

    fn match_op(&mut self, expected: &str) -> bool {
        if self.peek_op() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume(&mut self) -> Option<MilkRustToken> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn parse(&mut self) -> Result<f64, String> {
        if self.index == 0 {
            if let Some((assignment_index, operator)) =
                find_milkrust_top_level_assignment_token(self.tokens)
            {
                let target_tokens = &self.tokens[..assignment_index];
                let expression_tokens = &self.tokens[assignment_index + 1..];
                if !self.evaluate {
                    self.evaluate_arg_tokens(expression_tokens)?;
                    self.index = self.tokens.len();
                    return Ok(0.0);
                }
                let current = self
                    .lvalue_key(target_tokens)?
                    .map(|key| milkrust_number(&self.scope, &key))
                    .unwrap_or(0.0);
                let next = self.evaluate_arg_tokens(expression_tokens)?;
                let value = apply_milkrust_assignment_operator(current, operator, next);
                self.assign_arg_tokens(target_tokens, value)?;
                self.index = self.tokens.len();
                return Ok(value);
            }
        }
        let value = self.parse_conditional()?;
        if self.index < self.tokens.len() {
            return Err("Unexpected trailing MilkRust token".to_string());
        }
        Ok(value)
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.consume() {
            Some(MilkRustToken::Number(value)) => Ok(if self.evaluate { value } else { 0.0 }),
            Some(MilkRustToken::Op(op)) if op == "(" => {
                let value = self.parse_conditional()?;
                if !self.match_op(")") {
                    return Err("Unclosed MilkRust expression group.".to_string());
                }
                Ok(value)
            }
            Some(MilkRustToken::Ident(name)) => {
                if self.match_op("(") {
                    let args = self.remaining_call_args(&name)?;
                    if !self.evaluate {
                        for arg in args {
                            self.evaluate_arg_tokens(arg)?;
                        }
                        return Ok(0.0);
                    }
                    if let Some(value) = self.call_special_function(&name, &args)? {
                        return Ok(value);
                    }
                    let values = args
                        .iter()
                        .map(|arg| self.evaluate_arg_tokens(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.call_function(&name, &values)
                } else {
                    Ok(if self.evaluate {
                        match name.as_str() {
                            "e" => std::f64::consts::E,
                            "pi" => std::f64::consts::PI,
                            _ => milkrust_number(&self.scope, &name),
                        }
                    } else {
                        0.0
                    })
                }
            }
            Some(token) => Err(format!("Unexpected MilkRust token: {token:?}")),
            None => Err("Unexpected end of MilkRust expression.".to_string()),
        }
    }

    fn call_function(&mut self, name: &str, args: &[f64]) -> Result<f64, String> {
        let arg = |index: usize, default: f64| args.get(index).copied().unwrap_or(default);
        let out = match name {
            "abs" => arg(0, 0.0).abs(),
            "above" => (arg(0, 0.0) > arg(1, 0.0)) as i32 as f64,
            "acos" => arg(0, 0.0).clamp(-1.0, 1.0).acos(),
            "asin" => arg(0, 0.0).clamp(-1.0, 1.0).asin(),
            "atan" => arg(0, 0.0).atan(),
            "atan2" => arg(0, 0.0).atan2(arg(1, 0.0)),
            "below" => (arg(0, 0.0) < arg(1, 0.0)) as i32 as f64,
            "band" => (milkrust_truthy(arg(0, 0.0)) && milkrust_truthy(arg(1, 0.0))) as i32 as f64,
            "bor" => (milkrust_truthy(arg(0, 0.0)) || milkrust_truthy(arg(1, 0.0))) as i32 as f64,
            "bnot" => (arg(0, 0.0).abs() < 0.00001) as i32 as f64,
            "ceil" => arg(0, 0.0).ceil(),
            "cos" => arg(0, 0.0).cos(),
            "div" => {
                let right = arg(1, 0.0);
                if right.abs() < 0.00001 {
                    0.0
                } else {
                    arg(0, 0.0) / right
                }
            }
            "equal" => ((arg(0, 0.0) - arg(1, 0.0)).abs() < 0.00001) as i32 as f64,
            "exp" => arg(0, 0.0).exp(),
            "floor" => arg(0, 0.0).floor(),
            "gmegabuf" => milkrust_buffer_number(&self.scope, name, arg(0, 0.0)),
            "get_fft" => {
                let values = milkrust_frequency_data(&self.scope);
                milkrust_indexed_sample(&values, arg(0, 0.0))
            }
            "get_fft_hz" => {
                let sample_rate = milkrust_number(&self.scope, "sample_rate")
                    .max(milkrust_number(&self.scope, "samplerate"))
                    .max(44100.0);
                let nyquist = sample_rate / 2.0;
                let values = milkrust_frequency_data(&self.scope);
                milkrust_indexed_sample(
                    &values,
                    if nyquist > 0.0 {
                        arg(0, 0.0) / nyquist
                    } else {
                        0.0
                    },
                )
            }
            "get_waveform" => {
                let values = milkrust_waveform_data(&self.scope);
                milkrust_indexed_sample(&values, arg(0, 0.0))
            }
            "invsqrt" => {
                let value = arg(0, 0.0);
                let estimate = f64::from_bits(
                    0x5fe6_eb50_c7b5_37a9u64.wrapping_sub(value.to_bits() >> 1),
                );
                estimate * (1.5 - value * 0.5 * estimate * estimate)
            },
            "int" => arg(0, 0.0).trunc(),
            "log" => {
                if arg(0, 0.0) <= 0.0 {
                    0.0
                } else {
                    arg(0, 0.0).ln()
                }
            }
            "log10" => {
                if arg(0, 0.0) <= 0.0 {
                    0.0
                } else {
                    arg(0, 0.0).log10()
                }
            }
            "max" => arg(0, 0.0).max(arg(1, 0.0)),
            "megabuf" => milkrust_buffer_number(&self.scope, name, arg(0, 0.0)),
            "min" => arg(0, 0.0).min(arg(1, 0.0)),
            "mod" => {
                let right = arg(1, 0.0).trunc() as i64;
                if right == 0 {
                    0.0
                } else {
                    (arg(0, 0.0).trunc() as i64 % right) as f64
                }
            }
            "pow" => milkrust_pow(arg(0, 0.0), arg(1, 0.0)),
            "rand" => {
                let upper = arg(0, 1.0).trunc().max(0.0);
                if upper <= 0.0 {
                    0.0
                } else {
                    let counter = self.rand_counter;
                    self.rand_counter += 1;
                    (milkrust_pseudo_random_unit(&self.scope, counter) * upper)
                        .floor()
                        .min(upper - 1.0)
                }
            }
            "sign" => arg(0, 0.0).signum(),
            "sigmoid" => {
                let constraint = if arg(1, 1.0) == 0.0 { 1.0 } else { arg(1, 1.0) };
                1.0 / (1.0 + (-arg(0, 0.0) * constraint).exp())
            }
            "sin" => arg(0, 0.0).sin(),
            "sqr" => arg(0, 0.0) * arg(0, 0.0),
            "sqrt" => {
                if arg(0, 0.0) < 0.0 {
                    0.0
                } else {
                    arg(0, 0.0).sqrt()
                }
            }
            "tan" => arg(0, 0.0).tan(),
            _ => return Err(format!("Unsupported MilkRust function: {name}")),
        };
        Ok(if out.is_finite() { out } else { 0.0 })
    }

    fn parse_unary_sign(&mut self) -> Result<f64, String> {
        if self.match_op("+") {
            return self.parse_unary_sign();
        }
        if self.match_op("-") {
            return Ok(-self.parse_unary_sign()?);
        }
        if self.match_op("!") {
            let value = self.parse_power()?;
            return Ok(if self.evaluate {
                (value.abs() < 0.00001) as i32 as f64
            } else {
                0.0
            });
        }
        if self.match_op("~") {
            return Ok((!(self.parse_unary_sign()?.trunc() as i64)) as f64);
        }
        self.parse_primary()
    }

    fn parse_power_operand(&mut self) -> Result<f64, String> {
        if self.match_op("!") {
            let value = self.parse_power()?;
            return Ok(if self.evaluate {
                (value.abs() < 0.00001) as i32 as f64
            } else {
                0.0
            });
        }
        self.parse_unary_sign()
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let mut value = self.parse_power_operand()?;
        while self.match_op("^") {
            let right = self.parse_power_operand()?;
            value = milkrust_pow(value, right);
        }
        Ok(value)
    }

    fn parse_factor(&mut self) -> Result<f64, String> {
        let mut value = self.parse_power()?;
        while let Some(op) = self
            .peek_op()
            .filter(|op| matches!(*op, "*" | "/" | "%"))
            .map(str::to_string)
        {
            self.index += 1;
            let right = self.parse_power()?;
            value = match op.as_str() {
                "*" => value * right,
                "/" => {
                    if right.abs() < 0.00001 {
                        0.0
                    } else {
                        value / right
                    }
                }
                "%" => {
                    let divisor = right.trunc() as i64;
                    if divisor == 0 {
                        0.0
                    } else {
                        (value.trunc() as i64 % divisor) as f64
                    }
                }
                _ => value,
            };
        }
        Ok(value)
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut value = self.parse_factor()?;
        while let Some(op) = self
            .peek_op()
            .filter(|op| matches!(*op, "+" | "-"))
            .map(str::to_string)
        {
            self.index += 1;
            let right = self.parse_factor()?;
            value = if op == "+" {
                value + right
            } else {
                value - right
            };
        }
        Ok(value)
    }

    fn parse_shift(&mut self) -> Result<f64, String> {
        let mut value = self.parse_term()?;
        while let Some(op) = self
            .peek_op()
            .filter(|op| matches!(*op, "<<" | ">>"))
            .map(str::to_string)
        {
            self.index += 1;
            let right = self.parse_term()?;
            value = if op == "<<" {
                ((value.trunc() as i64) << (right.trunc() as u32)) as f64
            } else {
                ((value.trunc() as i64) >> (right.trunc() as u32)) as f64
            };
        }
        Ok(value)
    }

    fn parse_comparison(&mut self) -> Result<f64, String> {
        let mut value = self.parse_shift()?;
        while let Some(op) = self
            .peek_op()
            .filter(|op| matches!(*op, "<" | ">" | "<=" | ">=" | "==" | "!="))
            .map(str::to_string)
        {
            self.index += 1;
            let right = self.parse_shift()?;
            value = match op.as_str() {
                "<" => (value < right) as i32 as f64,
                ">" => (value > right) as i32 as f64,
                "<=" => (value <= right) as i32 as f64,
                ">=" => (value >= right) as i32 as f64,
                "==" => ((value - right).abs() < 0.00001) as i32 as f64,
                "!=" => ((value - right).abs() >= 0.00001) as i32 as f64,
                _ => value,
            };
        }
        Ok(value)
    }

    fn parse_bitwise_and(&mut self) -> Result<f64, String> {
        let mut value = self.parse_comparison()?;
        while self.match_op("&") {
            value = ((value.trunc() as i64) & (self.parse_comparison()?.trunc() as i64)) as f64;
        }
        Ok(value)
    }

    fn parse_bitwise_or(&mut self) -> Result<f64, String> {
        let mut value = self.parse_bitwise_and()?;
        while self.match_op("|") {
            value = ((value.trunc() as i64) | (self.parse_bitwise_and()?.trunc() as i64)) as f64;
        }
        Ok(value)
    }

    fn parse_logical_and(&mut self) -> Result<f64, String> {
        let mut value = self.parse_bitwise_or()?;
        while self.match_op("&&") {
            let outer_evaluate = self.evaluate;
            self.evaluate = outer_evaluate && milkrust_truthy(value);
            let right_result = self.parse_bitwise_or();
            self.evaluate = outer_evaluate;
            let right = right_result?;
            value = if outer_evaluate {
                (milkrust_truthy(value) && milkrust_truthy(right)) as i32 as f64
            } else {
                0.0
            };
        }
        Ok(value)
    }

    fn parse_logical_or(&mut self) -> Result<f64, String> {
        let mut value = self.parse_logical_and()?;
        while self.match_op("||") {
            let outer_evaluate = self.evaluate;
            self.evaluate = outer_evaluate && !milkrust_truthy(value);
            let right_result = self.parse_logical_and();
            self.evaluate = outer_evaluate;
            let right = right_result?;
            value = if outer_evaluate {
                (milkrust_truthy(value) || milkrust_truthy(right)) as i32 as f64
            } else {
                0.0
            };
        }
        Ok(value)
    }

    fn parse_conditional(&mut self) -> Result<f64, String> {
        let condition = self.parse_logical_or()?;
        if !self.match_op("?") {
            return Ok(condition);
        }
        let outer_evaluate = self.evaluate;
        let choose_true = milkrust_truthy(condition);
        self.evaluate = outer_evaluate && choose_true;
        let when_true_result = self.parse_conditional();
        self.evaluate = outer_evaluate;
        let when_true = when_true_result?;
        if !self.match_op(":") {
            return Err("Unclosed MilkRust conditional expression.".to_string());
        }
        self.evaluate = outer_evaluate && !choose_true;
        let when_false_result = self.parse_conditional();
        self.evaluate = outer_evaluate;
        let when_false = when_false_result?;
        Ok(if outer_evaluate {
            if choose_true { when_true } else { when_false }
        } else {
            0.0
        })
    }
}

fn milkrust_frequency_data(scope: &BTreeMap<String, MilkRustValue>) -> Vec<f64> {
    [
        "frequency_data",
        "frequencies",
        "frequency",
        "spectrum",
        "fft",
    ]
    .into_iter()
    .find_map(|name| match scope.get(name) {
        Some(MilkRustValue::Text(value)) => Some(
            value
                .split(',')
                .filter_map(|item| item.trim().parse::<f64>().ok())
                .collect::<Vec<_>>(),
        ),
        Some(MilkRustValue::Number(value)) => Some(vec![*value]),
        None => None,
    })
    .unwrap_or_default()
}

fn milkrust_waveform_data(scope: &BTreeMap<String, MilkRustValue>) -> Vec<f64> {
    ["waveform_data", "waveform", "samples", "wave"]
        .into_iter()
        .find_map(|name| match scope.get(name) {
            Some(MilkRustValue::Text(value)) => Some(
                value
                    .split(',')
                    .filter_map(|item| item.trim().parse::<f64>().ok())
                    .collect::<Vec<_>>(),
            ),
            Some(MilkRustValue::Number(value)) => Some(vec![*value]),
            None => None,
        })
        .unwrap_or_default()
}

fn find_milkrust_top_level_assignment_token(
    tokens: &[MilkRustToken],
) -> Option<(usize, &'static str)> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            MilkRustToken::Op(value) if value == "(" => depth += 1,
            MilkRustToken::Op(value) if value == ")" => depth = depth.saturating_sub(1),
            MilkRustToken::Op(value) if depth == 0 => match value.as_str() {
                "+=" => return Some((index, "+=")),
                "-=" => return Some((index, "-=")),
                "*=" => return Some((index, "*=")),
                "/=" => return Some((index, "/=")),
                "%=" => return Some((index, "%=")),
                "^=" => return Some((index, "^=")),
                "&=" => return Some((index, "&=")),
                "|=" => return Some((index, "|=")),
                "=" => return Some((index, "=")),
                _ => {}
            },
            _ => {}
        }
    }
    None
}

fn split_milkrust_call_tokens(tokens: &[MilkRustToken]) -> Vec<&[MilkRustToken]> {
    let mut args: Vec<&[MilkRustToken]> = Vec::new();
    let mut depth = 0usize;
    let mut arg_start = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        match token {
            MilkRustToken::Op(value) if value == "(" => {
                depth += 1;
            }
            MilkRustToken::Op(value) if value == ")" => {
                depth = depth.saturating_sub(1);
            }
            MilkRustToken::Op(value) if (value == "," || value == ";") && depth == 0 => {
                args.push(&tokens[arg_start..index]);
                arg_start = index + 1;
            }
            _ => {}
        }
    }
    if arg_start < tokens.len() {
        args.push(&tokens[arg_start..]);
    }
    args
}

/// Evaluate a single MilkDrop-compatible expression string and return the result.
///
/// Uses the default random counter of 0. For most use cases, this is sufficient
/// as long as you call it consistently within a single render frame.
///
/// # Arguments
///
/// * `expression` - A MilkRust expression string (e.g. `"sin(time) * bass"`)
/// * `variables` - A map of variable names to their current values
///
/// # Errors
///
/// Returns a [`MilkRustError`] if the expression cannot be parsed or evaluated.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use milkrust_expr::{evaluate_milkrust_expression, MilkRustValue};
///
/// let scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
/// let result = evaluate_milkrust_expression("2 + 3 * 4", &scope).unwrap();
/// assert_eq!(result, 14.0);
/// ```
pub fn evaluate_milkrust_expression(
    expression: &str,
    variables: &BTreeMap<String, MilkRustValue>,
) -> Result<f64, MilkRustError> {
    evaluate_milkrust_expression_with_rand_counter(expression, variables, 0)
        .map(|(value, _)| value)
}

/// Evaluate a single MilkDrop-compatible expression with explicit random counter.
///
/// This is useful when you need deterministic random values across multiple
/// expressions within a single frame. The random counter increments each time
/// the `rand()` function is called, so using the returned counter for subsequent
/// calls ensures unique random values.
///
/// Returns `(result, next_rand_counter)`.
///
/// # Errors
///
/// Returns a [`MilkRustError`] if the expression cannot be parsed or evaluated.
pub fn evaluate_milkrust_expression_with_rand_counter(
    expression: &str,
    variables: &BTreeMap<String, MilkRustValue>,
    rand_counter: usize,
) -> Result<(f64, usize), MilkRustError> {
    evaluate_milkrust_expression_with_scope(expression, variables, rand_counter)
        .map(|(value, _, rand_counter)| (value, rand_counter))
}

fn evaluate_milkrust_expression_with_scope(
    expression: &str,
    variables: &BTreeMap<String, MilkRustValue>,
    rand_counter: usize,
) -> Result<(f64, BTreeMap<String, MilkRustValue>, usize), MilkRustError> {
    let scope = variables
        .iter()
        .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let tokens = tokenize_milkrust_expression(expression)?;
    let mut parser = MilkRustExpressionParser::new(&tokens, scope, rand_counter, expression, true);
    let value = parser.parse().map_err(|e| MilkRustError::Parse {
        expression: expression.to_string(),
        message: e,
    })?;
    Ok((value, parser.scope, parser.rand_counter))
}

/// A pre-compiled MilkRust expression that can be evaluated repeatedly
/// without re-tokenizing or re-parsing the source string.
///
/// This is useful in real-time rendering contexts where the same expression
/// is evaluated every frame with different variable values.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use milkrust_expr::{CompiledMilkRustExpression, MilkRustValue};
///
/// let compiled = CompiledMilkRustExpression::compile("sin(time) + 0.5").unwrap();
/// let mut scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
/// scope.insert("time".to_string(), MilkRustValue::Number(0.0));
/// let result = compiled.evaluate(&scope).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledMilkRustExpression {
    tokens: Vec<MilkRustToken>,
    expression: String,
}

impl CompiledMilkRustExpression {
    /// Compile a MilkDrop expression string into a reusable expression.
    ///
    /// Tokenizes and validates the expression at compile time, so that
    /// subsequent evaluations skip parsing entirely.
    ///
    /// # Errors
    ///
    /// Returns a [`MilkRustError`] if the expression cannot be tokenized or parsed.
    pub fn compile(expression: &str) -> Result<Self, MilkRustError> {
        let tokens = tokenize_milkrust_expression(expression)?;
        // Validate by parsing with an empty scope (catches syntax errors)
        let mut parser = MilkRustExpressionParser::new(&tokens, BTreeMap::new(), 0, expression, true);
        parser.parse().map_err(|e| MilkRustError::Parse {
            expression: expression.to_string(),
            message: e,
        })?;
        Ok(Self {
            tokens,
            expression: expression.to_string(),
        })
    }

    /// Evaluate the compiled expression with the given variable scope.
    ///
    /// # Arguments
    ///
    /// * `variables` - A map of variable names to their current values
    ///
    /// # Errors
    ///
    /// Returns a [`MilkRustError`] if evaluation fails (e.g. unsupported function).
    pub fn evaluate(
        &self,
        variables: &BTreeMap<String, MilkRustValue>,
    ) -> Result<f64, MilkRustError> {
        self.evaluate_with_rand_counter(variables, 0)
            .map(|(value, _)| value)
    }

    /// Evaluate the compiled expression with an explicit random counter.
    ///
    /// The random counter ensures deterministic `rand()` calls across
    /// multiple expressions within a single frame.
    ///
    /// Returns `(result, next_rand_counter)`.
    pub fn evaluate_with_rand_counter(
        &self,
        variables: &BTreeMap<String, MilkRustValue>,
        rand_counter: usize,
    ) -> Result<(f64, usize), MilkRustError> {
        let scope = variables
            .iter()
            .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut parser = MilkRustExpressionParser::new(&self.tokens, scope, rand_counter, &self.expression, true);
        let value = parser.parse().map_err(|e| MilkRustError::Parse {
            expression: self.expression.clone(),
            message: e,
        })?;
        Ok((value, parser.rand_counter))
    }

    /// Evaluate the compiled expression, reusing a mutable scope buffer.
    pub fn evaluate_with_scope(
        &self,
        scope: &mut BTreeMap<String, MilkRustValue>,
        variables: &BTreeMap<String, MilkRustValue>,
        rand_counter: usize,
    ) -> Result<(f64, usize), MilkRustError> {
        scope.clear();
        for (key, value) in variables {
            scope.insert(key.to_ascii_lowercase(), value.clone());
        }
        let mut parser =
            MilkRustExpressionParser::new(&self.tokens, std::mem::take(scope), rand_counter, &self.expression, true);
        let value = parser.parse().map_err(|e| MilkRustError::Parse {
            expression: self.expression.clone(),
            message: e,
        })?;
        *scope = parser.scope;
        Ok((value, parser.rand_counter))
    }

    /// Return the original expression string.
    pub fn expression(&self) -> &str {
        &self.expression
    }
}

/// A pre-compiled block of MilkRust equation statements.
///
/// Analogous to [`CompiledMilkRustExpression`] but for multi-statement
/// equation blocks (init/update segments). Pre-splits statements and
/// pre-compiles each sub-expression so that evaluation skips tokenization
/// and parsing on subsequent calls.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use milkrust_expr::{CompiledMilkRustEquations, MilkRustValue};
///
/// let compiled = CompiledMilkRustEquations::compile("a = 1; b = a + 1").unwrap();
/// let mut scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
/// let result = compiled.evaluate(&scope).unwrap();
/// assert_eq!(result["a"], MilkRustValue::Number(1.0));
/// assert_eq!(result["b"], MilkRustValue::Number(2.0));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledMilkRustEquations {
    statements: Vec<CompiledEquationStatement>,
}

#[derive(Clone, Debug, PartialEq)]
enum CompiledEquationStatement {
    /// A buffer assignment: `megabuf(index_expr) OP= value_expr` or `gmegabuf(index_expr) OP= value_expr`
    BufferAssignment {
        buffer_name: String,
        index_expression: CompiledMilkRustExpression,
        operator: &'static str,
        value_expression: CompiledMilkRustExpression,
    },
    /// A regular assignment: `name OP= expression`
    Assignment {
        name: String,
        operator: &'static str,
        expression: CompiledMilkRustExpression,
    },
    /// A bare expression (e.g. function call with side effects)
    Expression(CompiledMilkRustExpression),
}

impl CompiledMilkRustEquations {
    /// Compile a block of MilkRust equation statements.
    ///
    /// Pre-tokenizes and pre-parses each sub-expression so that subsequent
    /// evaluations skip the parsing phase entirely.
    ///
    /// # Errors
    ///
    /// Returns a [`MilkRustError`] if any statement cannot be tokenized or parsed.
    pub fn compile(equations: &str) -> Result<Self, MilkRustError> {
        let statements = split_milkrust_equation_statements(equations);
        let mut compiled = Vec::with_capacity(statements.len());

        for statement in &statements {
            if let Some((buffer_name, index_expression, operator, expression)) =
                split_milkrust_buffer_assignment(statement)
            {
                let index_expr = CompiledMilkRustExpression::compile(index_expression)?;
                let value_expr = CompiledMilkRustExpression::compile(expression)?;
                compiled.push(CompiledEquationStatement::BufferAssignment {
                    buffer_name: buffer_name.to_ascii_lowercase(),
                    index_expression: index_expr,
                    operator,
                    value_expression: value_expr,
                });
                continue;
            }

            let Some((name, operator, expression)) = split_milkrust_assignment(statement) else {
                // Bare expression — compile and evaluate without assignment
                let expr = CompiledMilkRustExpression::compile(statement)?;
                compiled.push(CompiledEquationStatement::Expression(expr));
                continue;
            };

            let expr = CompiledMilkRustExpression::compile(expression)?;
            compiled.push(CompiledEquationStatement::Assignment {
                name: name.to_ascii_lowercase(),
                operator,
                expression: expr,
            });
        }

        Ok(Self { statements: compiled })
    }

    /// Evaluate the compiled equations with a fresh scope.
    ///
    /// # Arguments
    ///
    /// * `variables` - A map of initial variable values
    ///
    /// # Errors
    ///
    /// Returns a [`MilkRustError`] if evaluation fails.
    pub fn evaluate(
        &self,
        variables: &BTreeMap<String, MilkRustValue>,
    ) -> Result<BTreeMap<String, MilkRustValue>, MilkRustError> {
        let mut scope = variables
            .iter()
            .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut rand_counter = milkrust_number(&scope, "__rand_counter").max(0.0) as usize;

        for statement in &self.statements {
            match statement {
                CompiledEquationStatement::BufferAssignment {
                    buffer_name,
                    index_expression,
                    operator,
                    value_expression,
                } => {
                    let (buffer_index, next_rand) =
                        index_expression.evaluate_with_rand_counter(&scope, rand_counter)?;
                    rand_counter = next_rand;
                    let key = milkrust_buffer_key(buffer_name, buffer_index);
                    let current = milkrust_number(&scope, &key);
                    let (next, next_rand) =
                        value_expression.evaluate_with_rand_counter(&scope, rand_counter)?;
                    rand_counter = next_rand;
                    let value = apply_milkrust_assignment_operator(current, operator, next);
                    scope.insert(key, MilkRustValue::Number(value));
                }
                CompiledEquationStatement::Assignment {
                    name,
                    operator,
                    expression,
                } => {
                    let current = milkrust_number(&scope, name);
                    let (next, next_rand) =
                        expression.evaluate_with_rand_counter(&scope, rand_counter)?;
                    rand_counter = next_rand;
                    let value = apply_milkrust_assignment_operator(current, operator, next);
                    scope.insert(name.clone(), MilkRustValue::Number(value));
                }
                CompiledEquationStatement::Expression(expr) => {
                    let (_, next_rand) =
                        expr.evaluate_with_rand_counter(&scope, rand_counter)?;
                    rand_counter = next_rand;
                }
            }
        }

        scope.insert(
            "__rand_counter".to_string(),
            MilkRustValue::Number(rand_counter as f64),
        );
        Ok(scope)
    }

    /// Evaluate the compiled equations, reusing a mutable scope buffer.
    ///
    /// This is the most efficient evaluation method, as it avoids
    /// allocating a new [`BTreeMap`] on each call.
    ///
    /// # Arguments
    ///
    /// * `scope` - A mutable scope buffer that will be cleared and reused
    /// * `variables` - A map of initial variable values
    /// * `rand_counter` - Starting random counter for deterministic `rand()` calls
    ///
    /// # Errors
    ///
    /// Returns a [`MilkRustError`] if evaluation fails.
    pub fn evaluate_with_scope(
        &self,
        scope: &mut BTreeMap<String, MilkRustValue>,
        variables: &BTreeMap<String, MilkRustValue>,
        rand_counter: usize,
    ) -> Result<usize, MilkRustError> {
        scope.clear();
        for (key, value) in variables {
            scope.insert(key.to_ascii_lowercase(), value.clone());
        }

        let mut rand_counter = rand_counter;

        for statement in &self.statements {
            match statement {
                CompiledEquationStatement::BufferAssignment {
                    buffer_name,
                    index_expression,
                    operator,
                    value_expression,
                } => {
                    let (buffer_index, next_rand) =
                        index_expression.evaluate_with_rand_counter(scope, rand_counter)?;
                    rand_counter = next_rand;
                    let key = milkrust_buffer_key(buffer_name, buffer_index);
                    let current = milkrust_number(scope, &key);
                    let (next, next_rand) =
                        value_expression.evaluate_with_rand_counter(scope, rand_counter)?;
                    rand_counter = next_rand;
                    let value = apply_milkrust_assignment_operator(current, operator, next);
                    scope.insert(key, MilkRustValue::Number(value));
                }
                CompiledEquationStatement::Assignment {
                    name,
                    operator,
                    expression,
                } => {
                    let current = milkrust_number(scope, name);
                    let (next, next_rand) =
                        expression.evaluate_with_rand_counter(scope, rand_counter)?;
                    rand_counter = next_rand;
                    let value = apply_milkrust_assignment_operator(current, operator, next);
                    scope.insert(name.clone(), MilkRustValue::Number(value));
                }
                CompiledEquationStatement::Expression(expr) => {
                    let (_, next_rand) =
                        expr.evaluate_with_rand_counter(scope, rand_counter)?;
                    rand_counter = next_rand;
                }
            }
        }

        let final_rand_counter = milkrust_number(scope, "__rand_counter").max(0.0) as usize;
        scope.insert(
            "__rand_counter".to_string(),
            MilkRustValue::Number(final_rand_counter as f64),
        );
        Ok(final_rand_counter)
    }
}

/// Split a block of MilkRust equation statements into individual statements.
///
/// Parses a multi-line string of semicolon-separated or newline-separated
/// equation statements, respecting parentheses depth. Returns a vector of
/// trimmed, non-empty statement strings.
///
/// # Examples
///
/// ```
/// use milkrust_expr::split_milkrust_equation_statements;
///
/// let statements = split_milkrust_equation_statements("a = 1; b = 2\nc = 3");
/// assert_eq!(statements, vec!["a = 1", "b = 2", "c = 3"]);
/// ```
pub fn split_milkrust_equation_statements(equations: &str) -> Vec<String> {
    let sanitized = strip_milkrust_equation_comments(equations);
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in sanitized.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ';' | '\n' if depth == 0 => {
                let statement = current.trim();
                if !statement.is_empty() {
                    statements.push(statement.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let statement = current.trim();
    if !statement.is_empty() {
        statements.push(statement.to_string());
    }
    statements
}

/// Evaluate a block of MilkDrop-compatible equation statements.
///
/// Equations are semicolon-separated assignments that are evaluated
/// sequentially. Each assignment updates the variable scope for
/// subsequent statements. Supports `megabuf()` and `gmegabuf()` indexed
/// buffer assignments.
///
/// # Arguments
///
/// * `equations` - A multi-line string of equation statements
/// * `variables` - A map of initial variable values
///
/// # Errors
///
/// Returns a [`MilkRustError`] if any equation cannot be parsed or evaluated.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use milkrust_expr::evaluate_milkrust_equations;
///
/// let mut scope = BTreeMap::new();
/// scope.insert("a".to_string(), milkrust_expr::MilkRustValue::Number(1.0));
/// let result = evaluate_milkrust_equations("a = a + 1; a = a * 2", &scope).unwrap();
/// assert_eq!(result["a"], milkrust_expr::MilkRustValue::Number(4.0));
/// ```
pub fn evaluate_milkrust_equations(
    equations: &str,
    variables: &BTreeMap<String, MilkRustValue>,
) -> Result<BTreeMap<String, MilkRustValue>, MilkRustError> {
    let mut scope = variables
        .iter()
        .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut rand_counter = milkrust_number(&scope, "__rand_counter").max(0.0) as usize;
    for statement in split_milkrust_equation_statements(equations) {
        if let Some((buffer_name, index_expression, operator, expression)) =
            split_milkrust_buffer_assignment(&statement)
        {
            let (buffer_index, next_rand_counter) =
                evaluate_milkrust_expression_with_rand_counter(
                    index_expression,
                    &scope,
                    rand_counter,
                )?;
            rand_counter = next_rand_counter;
            let key = milkrust_buffer_key(&buffer_name, buffer_index);
            let current = milkrust_number(&scope, &key);
            let (next, next_rand_counter) =
                evaluate_milkrust_expression_with_rand_counter(expression, &scope, rand_counter)?;
            rand_counter = next_rand_counter;
            let value = apply_milkrust_assignment_operator(current, operator, next);
            scope.insert(key, MilkRustValue::Number(value));
            continue;
        }
        let Some((name, operator, expression)) = split_milkrust_assignment(&statement) else {
            let (_, next_scope, next_rand_counter) =
                evaluate_milkrust_expression_with_scope(&statement, &scope, rand_counter)?;
            scope = next_scope;
            rand_counter = next_rand_counter;
            continue;
        };
        let current = milkrust_number(&scope, &name);
        let (next, next_rand_counter) =
            evaluate_milkrust_expression_with_rand_counter(expression, &scope, rand_counter)?;
        rand_counter = next_rand_counter;
        let value = apply_milkrust_assignment_operator(current, operator, next);
        scope.insert(name, MilkRustValue::Number(value));
    }
    scope.insert(
        "__rand_counter".to_string(),
        MilkRustValue::Number(rand_counter as f64),
    );
    Ok(scope)
}

fn apply_milkrust_assignment_operator(current: f64, operator: &str, next: f64) -> f64 {
    match operator {
        "=" => next,
        "+=" => current + next,
        "-=" => current - next,
        "*=" => current * next,
        "/=" => {
            if next.abs() < 0.00001 { 0.0 } else { current / next }
        }
        "%=" => {
            let divisor = next.trunc() as i64;
            if divisor == 0 {
                0.0
            } else {
                (current.trunc() as i64 % divisor) as f64
            }
        }
        "^=" => milkrust_pow(current, next),
        "&=" => ((current.trunc() as i64) & (next.trunc() as i64)) as f64,
        "|=" => ((current.trunc() as i64) | (next.trunc() as i64)) as f64,
        _ => next,
    }
}

fn split_milkrust_buffer_assignment(
    statement: &str,
) -> Option<(String, &str, &'static str, &str)> {
    for operator in ["+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "="] {
        let Some((raw_name, expression)) = statement.split_once(operator) else {
            continue;
        };
        let raw_name = raw_name.trim();
        let open = raw_name.find('(')?;
        let close = raw_name.rfind(')')?;
        if close <= open || close != raw_name.len() - 1 {
            continue;
        }
        let name = raw_name[..open].trim().to_ascii_lowercase();
        if name != "megabuf" && name != "gmegabuf" {
            continue;
        }
        let index_expression = raw_name[open + 1..close].trim();
        if index_expression.is_empty() {
            continue;
        }
        return Some((name, index_expression, operator, expression.trim()));
    }
    None
}

fn split_milkrust_assignment(statement: &str) -> Option<(String, &'static str, &str)> {
    for operator in ["+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "="] {
        if let Some((raw_name, expression)) = statement.split_once(operator) {
            let name = raw_name.trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
                && name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            {
                return Some((name.to_ascii_lowercase(), operator, expression.trim()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milkrust_error_display_tokenize() {
        let err = MilkRustError::Tokenize {
            expression: "test".to_string(),
            position: 3,
            message: "bad char".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Tokenization error"));
        assert!(display.contains("position 3"));
    }

    #[test]
    fn milkrust_error_display_parse() {
        let err = MilkRustError::Parse {
            expression: "test".to_string(),
            message: "unclosed group".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Parse error"));
    }

    #[test]
    fn milkrust_error_display_evaluate() {
        let err = MilkRustError::Evaluate {
            function: "myfunc".to_string(),
            message: "bad argument".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Evaluation error"));
        assert!(display.contains("myfunc"));
    }

    #[test]
    fn milkrust_error_is_error() {
        let err: MilkRustError = MilkRustError::Parse {
            expression: "x".to_string(),
            message: "test".to_string(),
        };
        let _boxed: Box<dyn std::error::Error> = Box::new(err);
    }

    #[test]
    fn milkrust_value_display_number_integer() {
        assert_eq!(MilkRustValue::Number(42.0).to_string(), "42");
    }

    #[test]
    fn milkrust_value_display_number_float() {
        assert_eq!(MilkRustValue::Number(3.14).to_string(), "3.14");
    }

    #[test]
    fn milkrust_value_display_text() {
        assert_eq!(
            MilkRustValue::Text("hello".to_string()).to_string(),
            "hello"
        );
    }

    #[test]
    fn strip_comments_basic() {
        let input = "a = 1; // this is a comment\nb = 2";
        let result = strip_milkrust_equation_comments(input);
        assert_eq!(result, "a = 1; \nb = 2");
    }

    #[test]
    fn strip_comments_no_comment() {
        let input = "a = 1; b = 2";
        assert_eq!(strip_milkrust_equation_comments(input), input);
    }

    #[test]
    fn strip_comments_multiple_lines() {
        let input = "// full line comment\na = 1;\n// another comment";
        let result = strip_milkrust_equation_comments(input);
        assert_eq!(result, "\na = 1;\n");
    }

    #[test]
    fn tokenize_parses_identifiers_and_numbers() {
        let tokens = tokenize_milkrust_expression("sin(time) + 3.14");
        assert!(tokens.is_ok());
        let tokens = tokens.unwrap();
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0], MilkRustToken::Ident("sin".to_string()));
        assert_eq!(tokens[1], MilkRustToken::Op("(".to_string()));
        assert_eq!(tokens[2], MilkRustToken::Ident("time".to_string()));
        assert_eq!(tokens[3], MilkRustToken::Op(")".to_string()));
        assert_eq!(tokens[4], MilkRustToken::Op("+".to_string()));
        assert_eq!(tokens[5], MilkRustToken::Number(3.14));
    }

    #[test]
    fn tokenize_error_is_typed() {
        let err = tokenize_milkrust_expression("x + @").unwrap_err();
        assert!(matches!(err, MilkRustError::Tokenize { .. }));
    }

    #[test]
    fn evaluate_simple_expression() {
        let result = evaluate_milkrust_expression("2 + 3 * 4", &BTreeMap::new()).unwrap();
        assert!((result - 14.0).abs() < 0.0001);
    }

    #[test]
    fn evaluate_with_variables() {
        let mut scope = BTreeMap::new();
        scope.insert("time".to_string(), MilkRustValue::Number(1.0));
        scope.insert("bass".to_string(), MilkRustValue::Number(0.5));
        let result = evaluate_milkrust_expression("time + bass", &scope).unwrap();
        assert!((result - 1.5).abs() < 0.0001);
    }

    #[test]
    fn evaluate_with_functions() {
        let scope = BTreeMap::new();
        let result = evaluate_milkrust_expression("sin(0) + cos(0)", &scope).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }

    #[test]
    fn evaluate_assignment() {
        let mut scope = BTreeMap::new();
        scope.insert("a".to_string(), MilkRustValue::Number(10.0));
        let result = evaluate_milkrust_expression("b = a * 2", &scope).unwrap();
        assert!((result - 20.0).abs() < 0.0001);
    }

    #[test]
    fn evaluate_conditional() {
        let mut scope = BTreeMap::new();
        scope.insert("x".to_string(), MilkRustValue::Number(5.0));
        let result = evaluate_milkrust_expression("x > 3 ? 1 : 0", &scope).unwrap();
        assert!((result - 1.0).abs() < 0.0001);
    }


    #[test]
    fn evaluate_logical_operators_short_circuit() {
        let scope = BTreeMap::new();
        assert_eq!(evaluate_milkrust_expression("0 && 1", &scope).unwrap(), 0.0);
        assert_eq!(evaluate_milkrust_expression("1 || 0", &scope).unwrap(), 1.0);
        assert_eq!(
            evaluate_milkrust_expression("0 || 2 && 1", &scope).unwrap(),
            1.0
        );
        assert_eq!(
            evaluate_milkrust_expression("0 && unsupported(1)", &scope).unwrap(),
            0.0
        );
        assert_eq!(
            evaluate_milkrust_expression("1 || unsupported(1)", &scope).unwrap(),
            1.0
        );

        let result = evaluate_milkrust_equations(
            "x = 0 && assign(y, 1); z = 1 || assign(w, 1)",
            &scope,
        )
        .unwrap();
        assert_eq!(result["x"], MilkRustValue::Number(0.0));
        assert_eq!(result["z"], MilkRustValue::Number(1.0));
        assert!(!result.contains_key("y"));
        assert!(!result.contains_key("w"));
    }

    #[test]
    fn evaluate_equations() {
        let mut scope = BTreeMap::new();
        scope.insert("a".to_string(), MilkRustValue::Number(1.0));
        let result = evaluate_milkrust_equations("a = a + 1; a = a * 2", &scope).unwrap();
        assert_eq!(result["a"], MilkRustValue::Number(4.0));
    }

    #[test]
    fn split_equation_statements() {
        let stmts = split_milkrust_equation_statements("a = 1; b = 2\nc = 3");
        assert_eq!(stmts, vec!["a = 1", "b = 2", "c = 3"]);
    }

    #[test]
    fn compiled_expression_reuse() {
        let compiled = CompiledMilkRustExpression::compile("sin(time) + 0.5").unwrap();
        let mut scope = BTreeMap::new();
        scope.insert("time".to_string(), MilkRustValue::Number(0.0));
        let r1 = compiled.evaluate(&scope).unwrap();
        scope.insert("time".to_string(), MilkRustValue::Number(1.0));
        let r2 = compiled.evaluate(&scope).unwrap();
        assert_ne!(r1, r2);
    }

    #[test]
    fn compiled_equations_basic() {
        let compiled = CompiledMilkRustEquations::compile("a = 1; b = a + 1").unwrap();
        let scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
        let result = compiled.evaluate(&scope).unwrap();
        assert_eq!(result["a"], MilkRustValue::Number(1.0));
        assert_eq!(result["b"], MilkRustValue::Number(2.0));
    }

    #[test]
    fn compiled_equations_with_scope() {
        let compiled = CompiledMilkRustEquations::compile("a = 1; b = a + 1").unwrap();
        let mut scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
        scope.insert("a".to_string(), MilkRustValue::Number(5.0));
        let _ = compiled.evaluate_with_scope(&mut scope, &BTreeMap::new(), 0).unwrap();
        assert_eq!(scope["a"], MilkRustValue::Number(1.0));
        assert_eq!(scope["b"], MilkRustValue::Number(2.0));
    }

    #[test]
    fn compiled_equations_rand_counter() {
        let compiled = CompiledMilkRustEquations::compile("x = rand(2)").unwrap();
        let scope: BTreeMap<String, MilkRustValue> = BTreeMap::new();
        let result = compiled.evaluate(&scope).unwrap();
        assert!(result.contains_key("x"));
        assert!(result.contains_key("__rand_counter"));
    }

    #[test]
    fn evaluate_eel_power_boolean_lazy_and_memory_helpers() {
        let mut scope = BTreeMap::new();
        scope.insert("a".to_string(), MilkRustValue::Number(1.0));
        scope.insert("b".to_string(), MilkRustValue::Number(2.0));
        assert_eq!(evaluate_milkrust_expression("2 ^ 3 ^ 2", &scope).unwrap(), 64.0);
        assert_eq!(evaluate_milkrust_expression("-2 ^ 2", &scope).unwrap(), 4.0);
        assert_eq!(evaluate_milkrust_expression("2 ^ 3 * 2", &scope).unwrap(), 16.0);
        assert_eq!(evaluate_milkrust_expression("5 & 3", &scope).unwrap(), 1.0);
        assert_eq!(evaluate_milkrust_expression("5 | 2", &scope).unwrap(), 7.0);
        assert_eq!(evaluate_milkrust_expression("band(2, 4)", &scope).unwrap(), 1.0);
        assert_eq!(evaluate_milkrust_expression("bor(0, -2)", &scope).unwrap(), 1.0);
        assert_eq!(evaluate_milkrust_expression("bnot(0.000001)", &scope).unwrap(), 1.0);
        assert_eq!(evaluate_milkrust_expression("mod(7.5, 2.1)", &scope).unwrap(), 1.0);
        assert!((evaluate_milkrust_expression("invsqrt(4)", &scope).unwrap() - 0.5).abs() < 0.002);
        assert_eq!(evaluate_milkrust_expression("if(1, 7, unsupported(1))", &scope).unwrap(), 7.0);
        assert_eq!(evaluate_milkrust_expression("1 ? 9 : unsupported(1)", &scope).unwrap(), 9.0);

        let changed = evaluate_milkrust_equations(
            "if(1, assign(a, 3), assign(b, 4)); if(0, a, b)=10; x=2; x^=3; y=7; y&=3; z=7; z%=3; memset(8,0.25,2); freembuf(0); band(0,assign(side_effect,1))",
            &scope,
        )
        .unwrap();
        assert_eq!(changed["a"], MilkRustValue::Number(3.0));
        assert_eq!(changed["b"], MilkRustValue::Number(10.0));
        assert_eq!(changed["x"], MilkRustValue::Number(8.0));
        assert_eq!(changed["y"], MilkRustValue::Number(3.0));
        assert_eq!(changed["z"], MilkRustValue::Number(1.0));
        assert_eq!(changed["megabuf_8"], MilkRustValue::Number(0.25));
        assert_eq!(changed["megabuf_9"], MilkRustValue::Number(0.25));
        assert_eq!(changed["side_effect"], MilkRustValue::Number(1.0));
    }

}