// Static opaque arguments cannot be represented as ordinary ESSL values.
// Specialize resource helpers at their global bindings before method lowering.
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use super::{is_identifier, split_hlsl_call};

const MAX_SPECIALIZATIONS: usize = 64;
const MAX_EXPANSION_BYTES: usize = 512 * 1024;

#[derive(Clone)]
struct Function {
    name: String,
    span: Range<usize>,
    prefix: Vec<String>,
    parameters: Vec<Vec<String>>,
    suffix: Vec<String>,
    body: Option<Vec<String>>,
}

fn close_brace(tokens: &[String], open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token == "{" { depth += 1; }
        if token == "}" {
            depth = depth.checked_sub(1).ok_or("Unbalanced HLSL helper body")?;
            if depth == 0 { return Ok(index + 1); }
        }
    }
    Err("Unterminated HLSL helper body".into())
}

fn declarations(tokens: &[String]) -> Result<Vec<Function>, String> {
    let mut functions = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] == "{" { index = close_brace(tokens, index)?; continue; }
        if tokens[index] == ";" { index += 1; start = index; continue; }
        if tokens[index] != "(" || index == start || !is_identifier(&tokens[index - 1]) {
            index += 1; continue;
        }
        // Generic numeric/resource types may contain commas in declarations.
        let mut parameters = Vec::new();
        let mut parameter_start = index + 1;
        let mut cursor = index + 1;
        let (mut parens, mut brackets, mut angles) = (1i32, 0i32, 0i32);
        while cursor < tokens.len() {
            match tokens[cursor].as_str() {
                "(" => parens += 1,
                ")" => { parens -= 1; if parens == 0 { break; } }
                "[" => brackets += 1,
                "]" => brackets -= 1,
                "<" => angles += 1,
                ">" => angles -= 1,
                ">>" if angles >= 2 => angles -= 2,
                "," if parens == 1 && brackets == 0 && angles == 0 => {
                    parameters.push(tokens[parameter_start..cursor].to_vec()); parameter_start = cursor + 1;
                }
                _ => {}
            }
            cursor += 1;
        }
        if cursor == tokens.len() { return Err("Unterminated HLSL function declaration".into()); }
        if cursor > parameter_start { parameters.push(tokens[parameter_start..cursor].to_vec()); }
        if parameters.len() == 1 && parameters[0].len() == 1 && parameters[0][0] == "void" { parameters.clear(); }
        let close = cursor + 1;
        cursor = close;
        if tokens.get(cursor).map(String::as_str) == Some(":") { cursor += 2; }
        let definition = tokens.get(cursor).map(String::as_str) == Some("{");
        let prototype = tokens.get(cursor).map(String::as_str) == Some(";");
        if (definition || prototype) && !tokens[start..index].iter().any(|t| matches!(t.as_str(), "=" | ":")) {
            let end = if definition { close_brace(tokens, cursor)? } else { cursor + 1 };
            functions.push(Function {
                name: tokens[index - 1].clone(), span: start..end,
                prefix: tokens[start..index - 1].to_vec(), parameters,
                suffix: tokens[close..cursor].to_vec(),
                body: definition.then(|| tokens[cursor..end].to_vec()),
            });
            index = end; start = end;
        } else { index = close; }
    }
    Ok(functions)
}

fn opaque_name(parameter: &[String]) -> Result<Option<String>, String> {
    let first = parameter.iter().position(|t| !matches!(t.as_str(), "in" | "const" | "uniform" | "out" | "inout"));
    let Some(first) = first else { return Ok(None); };
    if !matches!(parameter[first].as_str(), "SamplerState" | "SamplerComparisonState" | "Texture1D" | "Texture2D" | "Texture3D"
        | "sampler" | "sampler2D" | "sampler3D") { return Ok(None); }
    if parameter[..first].iter().any(|t| matches!(t.as_str(), "out" | "inout")) {
        return Err("HLSL resource helper parameters must be read-only inputs".into());
    }
    let mut name = first + 1;
    if parameter.get(name).map(String::as_str) == Some("<") {
        let end = parameter[name + 1..].iter().position(|t| t == ">")
            .map(|n| name + 1 + n).ok_or("Unterminated resource helper element type")?;
        super::texture_element_width(&parameter[name + 1..end])?;
        name = end + 1;
    }
    if name + 1 != parameter.len() || !parameter.get(name).is_some_and(|t| is_identifier(t)) {
        return Err("Resource helper parameters require one named, non-array input without a default".into());
    }
    Ok(Some(parameter[name].clone()))
}

#[derive(Clone)]
struct Template { function: Function, opaque: Vec<Option<String>> }
// Match prototypes to definitions without depending on parameter names. Numeric
// overload resolution stays in the typed compiler after opaque substitution.
fn parameter_key(parameter: &[String]) -> Vec<String> {
    let end = parameter.iter().position(|t| matches!(t.as_str(), ":" | "=")).unwrap_or(parameter.len());
    let mut result = parameter[..end].to_vec();
    let mut direction = "in";
    while result.first().is_some_and(|t| matches!(t.as_str(), "in" | "out" | "inout" | "const" | "uniform")) {
        if result[0] == "out" { direction = "out"; }
        if result[0] == "inout" { direction = "inout"; }
        result.remove(0);
    }
    if result.first().map(String::as_str) == Some("unsigned") {
        result[0] = "uint".into();
        if result.get(1).map(String::as_str) == Some("int") { result.remove(1); }
    }
    let name_end = result.iter().position(|t| t == "[").unwrap_or(result.len());
    if name_end > 1 && is_identifier(&result[name_end - 1]) { result.remove(name_end - 1); }
    result.insert(0, direction.into());
    result
}
fn signature_key(function: &Function) -> Vec<Vec<String>> {
    function.parameters.iter().map(|p| parameter_key(p)).collect()
}
struct Instance { template: usize, name: String, signature: Vec<String>, body: Vec<String> }
struct Expansion<'a> {
    templates: BTreeMap<String, Vec<Template>>,
    globals: BTreeSet<String>,
    source: &'a [String],
    shadowed: BTreeSet<String>,
    instances: Vec<Instance>,
    keys: BTreeMap<(String, usize, Vec<Vec<String>>), Vec<usize>>,
    bytes: usize,
}

impl Expansion<'_> {
    fn binding(&self, expression: &[String]) -> bool {
        let Some(base) = expression.first() else { return false; };
        if !is_identifier(base) || self.shadowed.contains(base) || (!self.globals.contains(base) && !base.starts_with("sampler_")) { return false; }
        let mut index = 1;
        while index < expression.len() {
            if expression[index] == "." && expression.get(index + 1).is_some_and(|t| is_identifier(t)) { index += 2; }
            else if expression[index] == "[" && expression.get(index + 1).is_some_and(|t| t.parse::<usize>().is_ok())
                && expression.get(index + 2).map(String::as_str) == Some("]") { index += 3; }
            else { return false; }
        }
        true
    }

    fn expand(&mut self, tokens: &mut Vec<String>) -> Result<(), String> {
        let mut index = 0;
        while index + 1 < tokens.len() {
            if tokens[index + 1] != "(" || (index > 0 && tokens[index - 1] == ".") { index += 1; continue; }
            let Some(overloads) = self.templates.get(&tokens[index]).cloned() else { index += 1; continue; };
            let (arguments, end) = split_hlsl_call(tokens, index + 1)?;
            let candidates: Vec<_> = overloads.iter().filter(|t| t.opaque.len() == arguments.len()).collect();
            let Some(template) = candidates.first().copied() else {
                return Err(format!("No resource helper overload of {} accepts {} arguments", tokens[index], arguments.len()));
            };
            let mut bindings = Vec::new();
            let mut ordinary = Vec::new();
            for (argument, opaque) in arguments.iter().zip(&template.opaque) {
                if opaque.is_some() {
                    if !self.binding(argument) { return Err("Resource helper arguments must be global bindings or literal-indexed global fields/arrays".into()); }
                    bindings.push(argument.clone());
                } else { ordinary.push(argument.clone()); }
            }
            let key = (template.function.name.clone(), arguments.len(), bindings.clone());
            let instances = if let Some(instances) = self.keys.get(&key) { instances.clone() } else {
                if self.instances.len() + candidates.len() > MAX_SPECIALIZATIONS { return Err("HLSL resource helper specialization limit exceeded".into()); }
                let name = format!("milkrust_resource_helper_{}", self.instances.len());
                if self.source.iter().any(|token| token == &name) { return Err("Generated resource helper name collides with user code".into()); }
                let mut instances = Vec::new();
                for template in candidates {
                    let mut signature = template.function.prefix.clone(); signature.extend([name.clone(), "(".into()]);
                    let mut first = true;
                    for (parameter, opaque) in template.function.parameters.iter().zip(&template.opaque) {
                        if opaque.is_some() { continue; }
                        if !first { signature.push(",".into()); } first = false;
                        signature.extend(parameter.iter().cloned());
                    }
                    signature.push(")".into()); signature.extend(template.function.suffix.iter().cloned());
                    for (parameter, opaque) in template.function.parameters.iter().zip(&template.opaque) {
                        if opaque.is_none() && parameter.iter().any(|name| bindings.iter().any(|binding| binding.first() == Some(name))) {
                            return Err("A value parameter hides a specialized resource binding".into());
                        }
                    }
                    let replacements: BTreeMap<_, _> = template.opaque.iter().filter_map(Option::as_ref).cloned().zip(bindings.iter().cloned()).collect();
                    let original = template.function.body.as_ref().ok_or("Resource helper has no definition")?;
                    for value in replacements.values() {
                        let root = &value[0];
                        if !replacements.contains_key(root) && original.iter().any(|token| token == root) {
                            return Err("A resource helper binding would capture an existing body identifier".into());
                        }
                    }
                    let mut body = Vec::new();
                    for (position, token) in original.iter().enumerate() {
                        if let Some(value) = replacements.get(token).filter(|_| position == 0 || original[position - 1] != ".") {
                            if position > 0 && is_identifier(&original[position - 1]) && original[position - 1] != "return" {
                                return Err("Shadowed resource helper parameters are unsupported".into());
                            }
                            if original.get(position + 1).is_some_and(|t| matches!(t.as_str(), "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" | "++" | "--"))
                                || position > 0 && matches!(original[position - 1].as_str(), "++" | "--") {
                                return Err("Resource helper arguments cannot be reassigned".into());
                            }
                            body.extend(value.iter().cloned());
                        } else { body.push(token.clone()); }
                    }
                    self.bytes = self.bytes.saturating_add(signature.iter().chain(&body).map(|t| t.len() + 1).sum::<usize>());
                    if self.bytes > MAX_EXPANSION_BYTES { return Err("HLSL resource helper expansion exceeds 512 KiB".into()); }
                    if self.instances.len() >= MAX_SPECIALIZATIONS { return Err("HLSL resource helper specialization limit exceeded".into()); }
                    let instance = self.instances.len();
                    self.instances.push(Instance { template: template.function.span.start, name: name.clone(), signature, body });
                    instances.push(instance);
                }
                // Publish the whole overload set before forwarding calls. An
                // overload may call another overload of the same name. The
                // typed compiler checks cycles after resolving those calls.
                self.keys.insert(key, instances.clone());
                for &instance in &instances {
                    let mut body = std::mem::take(&mut self.instances[instance].body);
                    self.expand(&mut body)?;
                    self.instances[instance].body = body;
                }
                instances
            };
            let mut call = vec![self.instances[instances[0]].name.clone(), "(".into()];
            for (argument, values) in ordinary.into_iter().enumerate() {
                if argument > 0 { call.push(",".into()); } call.extend(values);
            }
            call.push(")".into()); tokens.splice(index..end, call); index += 1;
        }
        Ok(())
    }
}

pub(super) fn specialize(tokens: &mut Vec<String>) -> Result<(), String> {
    // Preserve the existing typed opaque-parameter path (including overloads)
    // when no initialized state needs forwarding through a global alias.
    let has_static_state = tokens.iter().enumerate().any(|(index, token)| token == "sampler_state"
        || matches!(token.as_str(), "SamplerState" | "SamplerComparisonState")
            && matches!(tokens.get(index + 2).map(String::as_str), Some("{") | Some("=")));
    if !has_static_state { return Ok(()); }
    let functions = declarations(tokens)?;
    let mut templates = BTreeMap::<String, Vec<Template>>::new();
    for function in functions.iter().filter(|f| f.body.is_some()) {
        let opaque = function.parameters.iter().map(|p| opaque_name(p)).collect::<Result<Vec<_>, _>>()?;
        if opaque.iter().any(Option::is_some) {
            templates.entry(function.name.clone()).or_default().push(Template { function: function.clone(), opaque });
        }
    }
    if templates.is_empty() { return Ok(()); }
    for (name, overloads) in &templates {
        for function in functions.iter().filter(|f| &f.name == name) {
            if function.body.is_some() && !overloads.iter().any(|t| t.function.span.start == function.span.start) {
                return Err("Mixed resource and ordinary helper overloads require distinct names".into());
            }
            if function.body.is_none() && !overloads.iter().any(|t| signature_key(&t.function) == signature_key(function)) {
                return Err(format!("Resource helper prototype {name} has no matching definition"));
            }
        }
        for (index, first) in overloads.iter().enumerate() {
            for other in &overloads[index + 1..] {
                if first.opaque.len() != other.opaque.len() { continue; }
                for (position, (left, right)) in first.opaque.iter().zip(&other.opaque).enumerate() {
                    if left.is_some() != right.is_some() || left.is_some()
                        && parameter_key(&first.function.parameters[position]) != parameter_key(&other.function.parameters[position]) {
                        return Err("Resource overloads of one arity must use the same opaque parameter types and positions".into());
                    }
                }
            }
        }
    }
    let mut globals = BTreeSet::new();
    let mut index = 0;
    while index < tokens.len() {
        if let Some(function) = functions.iter().find(|f| f.span.start == index) { index = function.span.end; continue; }
        if tokens[index] == "{" { index = close_brace(tokens, index)?; continue; }
        if is_identifier(&tokens[index]) { globals.insert(tokens[index].clone()); }
        index += 1;
    }
    // Substitution captures global opaque bindings; reject local declarations
    // that could hide one instead of silently choosing the wrong texture/state.
    let mut shadowed = BTreeSet::new();
    for function in functions.iter().filter(|f| !templates.contains_key(&f.name)) {
        for parameter in &function.parameters {
            let end = parameter.iter().position(|t| matches!(t.as_str(), ":" | "=" | "[")).unwrap_or(parameter.len());
            if let Some(name) = parameter[..end].last() { shadowed.insert(name.clone()); }
        }
        if let Some(body) = &function.body {
            for pair in body.windows(2) {
                if is_identifier(&pair[0]) && pair[0] != "return" && is_identifier(&pair[1]) { shadowed.insert(pair[1].clone()); }
            }
        }
    }
    for index in 0..tokens.len().saturating_sub(1) {
        if tokens[index] == "shader_body" && tokens[index + 1] == "{" {
            let end = close_brace(tokens, index + 1)?;
            for pair in tokens[index + 2..end - 1].windows(2) {
                if is_identifier(&pair[0]) && pair[0] != "return" && is_identifier(&pair[1]) { shadowed.insert(pair[1].clone()); }
            }
        }
    }
    let source = tokens.clone();
    let mut expansion = Expansion { templates, globals, source: &source, shadowed, instances: Vec::new(), keys: BTreeMap::new(), bytes: 0 };
    let mut slots = Vec::new();
    for function in functions.iter().rev().filter(|f| expansion.templates.contains_key(&f.name)) {
        let slot = format!("milkrust_resource_slot_{}", slots.len());
        if source.iter().any(|token| token == &slot) { return Err("Generated resource slot collides with user code".into()); }
        let definitions: Vec<_> = expansion.templates[&function.name].iter().filter(|t|
            if function.body.is_some() { t.function.span.start == function.span.start }
            else { signature_key(&t.function) == signature_key(function) }
        ).map(|t| t.function.span.start).collect();
        slots.push((slot.clone(), definitions, function.body.is_some()));
        tokens.splice(function.span.clone(), [slot]);
    }
    expansion.expand(tokens)?;
    let mut result = Vec::new();
    for token in tokens.iter() {
        if let Some((_, templates, definition)) = slots.iter().find(|(slot, _, _)| slot == token) {
            for instance in expansion.instances.iter().filter(|i| templates.contains(&i.template)) {
                result.extend(instance.signature.iter().cloned());
                if *definition { result.extend(instance.body.iter().cloned()); } else { result.push(";".into()); }
            }
        } else { result.push(token.clone()); }
    }
    if result.iter().map(|t| t.len() + 1).sum::<usize>() > MAX_EXPANSION_BYTES {
        return Err("HLSL resource helper output exceeds 512 KiB".into());
    }
    *tokens = result;
    Ok(())
}
