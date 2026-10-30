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

fn resource_struct_types(tokens: &[String]) -> BTreeSet<String> {
    let mut definitions = Vec::<(String, usize, usize)>::new();
    let mut index = 0usize;
    let mut brace_depth = 0i32;
    while index + 2 < tokens.len() {
        if tokens[index] == "{" { brace_depth += 1; index += 1; continue; }
        if tokens[index] == "}" { brace_depth -= 1; index += 1; continue; }
        if brace_depth == 0 && tokens[index] == "struct"
            && tokens.get(index + 1).is_some_and(|name| is_identifier(name)) {
            let name = tokens[index + 1].clone();
            let Some(open_offset) = tokens[index + 2..].iter().position(|token| token == "{") else {
                index += 2;
                continue;
            };
            let open = index + 2 + open_offset;
            let mut nested = 1i32;
            let mut close = open + 1;
            while close < tokens.len() && nested > 0 {
                if tokens[close] == "{" { nested += 1; }
                if tokens[close] == "}" { nested -= 1; }
                close += 1;
            }
            if nested == 0 {
                definitions.push((name, open, close));
                index = close;
                continue;
            }
        }
        index += 1;
    }

    let opaque_types = ["Texture1D", "Texture2D", "Texture3D", "SamplerState",
        "SamplerComparisonState", "sampler", "sampler2D", "sampler3D"];
    let mut structures = BTreeSet::new();
    for (name, open, close) in &definitions {
        if tokens[*open + 1..*close - 1].iter().any(|token| opaque_types.contains(&token.as_str())) {
            structures.insert(name.clone());
        }
    }
    for _ in 0..=definitions.len() {
        let mut additions = Vec::new();
        for (parent, open, close) in &definitions {
            if structures.contains(parent) { continue; }
            let mut statement_start = open + 1;
            while statement_start < close - 1 {
                let Some(end_offset) = tokens[statement_start..*close - 1].iter().position(|token| token == ";") else {
                    break;
                };
                let statement_end = statement_start + end_offset;
                let type_index = if tokens.get(statement_start).map(String::as_str) == Some("uniform") {
                    statement_start + 1
                } else {
                    statement_start
                };
                if tokens.get(type_index).is_some_and(|ty| structures.contains(ty))
                    && type_index + 2 == statement_end
                    && tokens.get(type_index + 1).is_some_and(|name| is_identifier(name)) {
                    additions.push(parent.clone());
                    break;
                }
                statement_start = statement_end + 1;
            }
        }
        if additions.is_empty() { break; }
        structures.extend(additions);
    }
    structures
}

fn resource_only_struct_types(tokens: &[String]) -> BTreeSet<String> {
    let mut definitions = Vec::<(String, usize, usize)>::new();
    let mut index = 0usize;
    let mut brace_depth = 0i32;
    while index + 2 < tokens.len() {
        if tokens[index] == "{" { brace_depth += 1; index += 1; continue; }
        if tokens[index] == "}" { brace_depth -= 1; index += 1; continue; }
        if brace_depth == 0 && tokens[index] == "struct"
            && tokens.get(index + 1).is_some_and(|name| is_identifier(name)) {
            let name = tokens[index + 1].clone();
            let Some(open_offset) = tokens[index + 2..].iter().position(|token| token == "{") else {
                index += 2;
                continue;
            };
            let open = index + 2 + open_offset;
            let mut nested = 1i32;
            let mut close = open + 1;
            while close < tokens.len() && nested > 0 {
                if tokens[close] == "{" { nested += 1; }
                if tokens[close] == "}" { nested -= 1; }
                close += 1;
            }
            if nested == 0 {
                definitions.push((name, open, close));
                index = close;
                continue;
            }
        }
        index += 1;
    }

    let opaque_types = ["Texture1D", "Texture2D", "Texture3D", "SamplerState",
        "SamplerComparisonState", "sampler", "sampler2D", "sampler3D"];
    let mut resource_only = BTreeSet::new();
    for _ in 0..=definitions.len() {
        let mut additions = Vec::new();
        for (name, open, close) in &definitions {
            if resource_only.contains(name) { continue; }
            let body = &tokens[*open + 1..*close - 1];
            let mut statement_start = 0usize;
            let mut valid = true;
            let mut has_field = false;
            while statement_start < body.len() {
                let Some(end_offset) = body[statement_start..].iter().position(|token| token == ";") else {
                    valid = false;
                    break;
                };
                let statement = &body[statement_start..statement_start + end_offset];
                statement_start += end_offset + 1;
                let mut cursor = if statement.first().map(String::as_str) == Some("uniform") { 1 } else { 0 };
                if cursor >= statement.len() { valid = false; break; }
                let ty = statement[cursor].as_str();
                let mut type_end = cursor + 1;
                if statement.get(type_end).map(String::as_str) == Some("<") {
                    let mut angle_depth = 0i32;
                    while type_end < statement.len() {
                        if statement[type_end] == "<" { angle_depth += 1; }
                        if statement[type_end] == ">" { angle_depth -= 1; }
                        if statement[type_end] == ">>" { angle_depth -= 2; }
                        type_end += 1;
                        if angle_depth <= 0 { break; }
                    }
                    if angle_depth > 0 { valid = false; break; }
                }
                let allowed_type = opaque_types.contains(&ty) || resource_only.contains(ty);
                if !allowed_type { valid = false; break; }
                cursor = type_end;
                let mut declarators = 0usize;
                loop {
                    if !statement.get(cursor).is_some_and(|field| is_identifier(field)) {
                        valid = false;
                        break;
                    }
                    cursor += 1;
                    while statement.get(cursor).map(String::as_str) == Some("[") {
                        cursor += 1;
                        let mut depth = 1i32;
                        while cursor < statement.len() && depth > 0 {
                            if statement[cursor] == "[" { depth += 1; }
                            if statement[cursor] == "]" { depth -= 1; }
                            cursor += 1;
                        }
                        if depth != 0 { valid = false; break; }
                    }
                    if !valid { break; }
                    declarators += 1;
                    if cursor == statement.len() { break; }
                    if statement[cursor] != "," { valid = false; break; }
                    cursor += 1;
                }
                if !valid || declarators == 0 { valid = false; break; }
                has_field = true;
            }
            if valid && has_field && statement_start == body.len() {
                additions.push(name.clone());
            }
        }
        if additions.is_empty() { break; }
        resource_only.extend(additions);
    }
    resource_only
}

fn global_resource_struct_instances(
    tokens: &[String],
    functions: &[Function],
    resource_structs: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut instances = BTreeMap::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if let Some(function) = functions.iter().find(|function| function.span.start == index) {
            index = function.span.end;
            continue;
        }
        if tokens[index] == "{" {
            if let Ok(end) = close_brace(tokens, index) { index = end; continue; }
        }
        if resource_structs.contains(&tokens[index])
            && tokens.get(index + 1).is_some_and(|name| is_identifier(name))
            && matches!(tokens.get(index + 2).map(String::as_str), Some(";") | Some("=")) {
            instances.insert(tokens[index + 1].clone(), tokens[index].clone());
        }
        index += 1;
    }
    instances
}

fn declaration_of_name(tokens: &[String], index: usize, name: &str) -> bool {
    if tokens.get(index).map(String::as_str) != Some(name)
        || index == 0 || !is_identifier(&tokens[index - 1])
        || matches!(tokens[index - 1].as_str(), "return" | "if" | "while" | "for" | "switch" | "case" | "discard")
        || tokens[index - 1] == "."
    {
        return false;
    }
    matches!(tokens.get(index + 1).map(String::as_str), Some(";") | Some("=") | Some("[") | Some(","))
}

fn access_chain_end(tokens: &[String], start: usize) -> usize {
    let mut index = start + 1;
    loop {
        if tokens.get(index).map(String::as_str) == Some(".")
            && tokens.get(index + 1).is_some_and(|field| is_identifier(field)) {
            index += 2;
            continue;
        }
        if tokens.get(index).map(String::as_str) == Some("[") {
            index += 1;
            let mut depth = 1i32;
            while index < tokens.len() && depth > 0 {
                if tokens[index] == "[" { depth += 1; }
                if tokens[index] == "]" { depth -= 1; }
                index += 1;
            }
            if depth == 0 { continue; }
        }
        return index;
    }
}


#[derive(Clone)]
struct GlobalResource {
    ty: Vec<String>,
    array_len: Option<usize>,
}

fn resource_type_signature(tokens: &[String], start: usize) -> Option<(Vec<String>, usize)> {
    let opaque = ["Texture1D", "Texture2D", "Texture3D", "SamplerState",
        "SamplerComparisonState", "sampler", "sampler2D", "sampler3D"];
    if !tokens.get(start).is_some_and(|ty| opaque.contains(&ty.as_str())) {
        return None;
    }
    let mut end = start + 1;
    if tokens.get(end).map(String::as_str) == Some("<") {
        let mut angle_depth = 0i32;
        while end < tokens.len() {
            if tokens[end] == "<" { angle_depth += 1; }
            if tokens[end] == ">" { angle_depth -= 1; }
            if tokens[end] == ">>" { angle_depth -= 2; }
            end += 1;
            if angle_depth <= 0 { break; }
        }
        if angle_depth != 0 { return None; }
    }
    Some((tokens[start..end].to_vec(), end))
}

fn global_resource_declarations(tokens: &[String], functions: &[Function]) -> BTreeMap<String, GlobalResource> {
    let mut resources = BTreeMap::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if let Some(function) = functions.iter().find(|function| function.span.start == index) {
            index = function.span.end;
            continue;
        }
        if tokens[index] == "{" {
            if let Ok(end) = close_brace(tokens, index) { index = end; continue; }
        }
        let Some((ty, name_index)) = resource_type_signature(tokens, index) else {
            index += 1;
            continue;
        };
        if !tokens.get(name_index).is_some_and(|name| is_identifier(name)) {
            index += 1;
            continue;
        }
        let name = tokens[name_index].clone();
        let mut cursor = name_index + 1;
        let mut array_len = None;
        if tokens.get(cursor).map(String::as_str) == Some("[") {
            let Some(length) = tokens.get(cursor + 1).and_then(|value| value.parse::<usize>().ok()) else {
                index += 1;
                continue;
            };
            if tokens.get(cursor + 2).map(String::as_str) != Some("]") || length == 0 {
                index += 1;
                continue;
            }
            array_len = Some(length);
            cursor += 3;
        }
        if matches!(tokens.get(cursor).map(String::as_str), Some(";") | Some("=") | Some("{")) {
            resources.insert(name, GlobalResource { ty, array_len });
        }
        index = name_index + 1;
    }
    resources
}

fn resource_binding_matches(
    expression: &[String],
    expected_type: &[String],
    globals: &BTreeMap<String, GlobalResource>,
) -> bool {
    let Some(root) = expression.first().filter(|root| is_identifier(root)) else { return false; };
    let Some(global) = globals.get(root) else { return false; };
    if global.ty.as_slice() != expected_type { return false; }
    match expression {
        [_] => global.array_len.is_none(),
        [_, open, index, close] if open == "[" && close == "]" => {
            index.parse::<usize>().ok().is_some_and(|index| global.array_len.is_some_and(|length| index < length))
        }
        _ => false,
    }
}

fn split_initializer_values(tokens: &[String]) -> Option<Vec<Vec<String>>> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let (mut parens, mut brackets, mut angles, mut braces) = (0i32, 0i32, 0i32, 0i32);
    for (index, token) in tokens.iter().enumerate() {
        match token.as_str() {
            "(" => parens += 1, ")" => parens -= 1,
            "[" => brackets += 1, "]" => brackets -= 1,
            "<" => angles += 1, ">" => angles -= 1,
            ">>" if angles >= 2 => angles -= 2,
            "{" => braces += 1, "}" => braces -= 1,
            "," if parens == 0 && brackets == 0 && angles == 0 && braces == 0 => {
                if index == start { return None; }
                result.push(tokens[start..index].to_vec());
                start = index + 1;
            }
            _ => {}
        }
        if parens < 0 || brackets < 0 || angles < 0 || braces < 0 { return None; }
    }
    if start < tokens.len() {
        result.push(tokens[start..].to_vec());
    } else if !tokens.is_empty() {
        return None;
    }
    Some(result)
}

fn normalize_static_local_resource_arrays(tokens: &mut Vec<String>) -> Result<(), String> {
    let functions = declarations(tokens)?;
    let globals = global_resource_declarations(tokens, &functions);
    if globals.is_empty() { return Ok(()); }
    let mut scopes = Vec::<(Range<usize>, Option<usize>)>::new();
    for (function_index, function) in functions.iter().enumerate() {
        let Some(body) = &function.body else { continue; };
        let body_start = function.span.end.checked_sub(body.len()).ok_or("Invalid HLSL function body range")?;
        scopes.push((body_start..function.span.end, Some(function_index)));
    }
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if tokens[index] == "shader_body" && tokens[index + 1] == "{" {
            let end = close_brace(tokens, index + 1)?;
            scopes.push((index + 1..end, None));
            index = end;
        } else {
            index += 1;
        }
    }

    let mut edits = Vec::<(Range<usize>, Vec<String>)>::new();
    for (scope, function_index) in scopes {
        let mut depth = 0i32;
        let mut candidates = Vec::<(usize, usize, String, Vec<Vec<String>>)>::new();
        let mut index = scope.start;
        while index < scope.end {
            if tokens[index] == "{" { depth += 1; index += 1; continue; }
            if tokens[index] == "}" { depth -= 1; index += 1; continue; }
            let Some((ty, name_index)) = resource_type_signature(tokens, index) else {
                index += 1;
                continue;
            };
            if depth != 1 || !tokens.get(name_index).is_some_and(|name| is_identifier(name))
                || tokens.get(name_index + 1).map(String::as_str) != Some("[") {
                index += 1;
                continue;
            }
            let name = tokens[name_index].clone();
            let Some(length) = tokens.get(name_index + 2).and_then(|value| value.parse::<usize>().ok()) else {
                index += 1;
                continue;
            };
            if length == 0 || length > 15
                || tokens.get(name_index + 3).map(String::as_str) != Some("]")
                || tokens.get(name_index + 4).map(String::as_str) != Some("=")
                || tokens.get(name_index + 5).map(String::as_str) != Some("{") {
                index += 1;
                continue;
            }
            let initializer_open = name_index + 5;
            let mut initializer_depth = 1i32;
            let mut initializer_close = initializer_open + 1;
            while initializer_close < scope.end && initializer_depth > 0 {
                if tokens[initializer_close] == "{" { initializer_depth += 1; }
                if tokens[initializer_close] == "}" { initializer_depth -= 1; }
                initializer_close += 1;
            }
            if initializer_depth != 0
                || tokens.get(initializer_close).map(String::as_str) != Some(";") {
                index += 1;
                continue;
            }
            let values = split_initializer_values(&tokens[initializer_open + 1..initializer_close - 1]);
            let Some(values) = values.filter(|values| values.len() == length
                && values.iter().all(|value| resource_binding_matches(value, &ty, &globals))) else {
                index += 1;
                continue;
            };
            candidates.push((index, initializer_close + 1, name, values));
            index = initializer_close + 1;
        }
        if candidates.is_empty() { continue; }

        let mut counts = BTreeMap::<String, usize>::new();
        for (_, _, name, _) in &candidates { *counts.entry(name.clone()).or_default() += 1; }
        let parameter_names: BTreeSet<String> = function_index
            .and_then(|function_index| functions.get(function_index))
            .into_iter()
            .flat_map(|function| function.parameters.iter())
            .filter_map(|parameter| {
                let end = parameter.iter().position(|token| matches!(token.as_str(), ":" | "=" | "["))
                    .unwrap_or(parameter.len());
                parameter[..end].last().filter(|name| is_identifier(name)).cloned()
            })
            .collect();

        for (declaration_start, declaration_end, name, values) in candidates {
            if counts.get(&name).copied() != Some(1) || parameter_names.contains(&name) { continue; }
            if (scope.start..scope.end).any(|index| {
                (index < declaration_start || index >= declaration_end)
                    && declaration_of_name(tokens, index, &name)
            }) {
                continue;
            }
            let roots_are_global = values.iter().all(|value| {
                value.first().is_some_and(|root| !parameter_names.contains(root))
                    && !(scope.start..scope.end).any(|index| declaration_of_name(tokens, index, &value[0]))
            });
            if !roots_are_global { continue; }

            let mut uses = Vec::<(Range<usize>, Vec<String>)>::new();
            let mut unsupported_use = false;
            for index in scope.clone() {
                if (declaration_start..declaration_end).contains(&index)
                    || tokens[index] != name || (index > 0 && tokens[index - 1] == ".") {
                    continue;
                }
                let Some(array_index) = tokens.get(index + 2).and_then(|value| value.parse::<usize>().ok()) else {
                    unsupported_use = true;
                    break;
                };
                if tokens.get(index + 1).map(String::as_str) != Some("[")
                    || tokens.get(index + 3).map(String::as_str) != Some("]")
                    || array_index >= values.len() {
                    unsupported_use = true;
                    break;
                }
                let chain_end = access_chain_end(tokens, index);
                if matches!(tokens.get(chain_end).map(String::as_str), Some("=") | Some("+=") | Some("-=")
                    | Some("*=") | Some("/=") | Some("%=") | Some("&=") | Some("|=")
                    | Some("^=") | Some("<<=") | Some(">>=") | Some("++") | Some("--"))
                    || index > scope.start && matches!(tokens[index - 1].as_str(), "++" | "--") {
                    unsupported_use = true;
                    break;
                }
                uses.push((index..index + 4, values[array_index].clone()));
            }
            if unsupported_use { continue; }
            edits.push((declaration_start..declaration_end, Vec::new()));
            edits.extend(uses);
        }
    }

    edits.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in edits { tokens.splice(range, replacement); }
    Ok(())
}

fn normalize_static_local_resource_aliases(tokens: &mut Vec<String>) -> Result<(), String> {
    let functions = declarations(tokens)?;
    let resource_only = resource_only_struct_types(tokens);
    if resource_only.is_empty() { return Ok(()); }
    let globals = global_resource_struct_instances(tokens, &functions, &resource_only);
    if globals.is_empty() { return Ok(()); }

    let mut scopes = Vec::<(Range<usize>, Option<usize>)>::new();
    for (function_index, function) in functions.iter().enumerate() {
        let Some(body) = &function.body else { continue; };
        let body_start = function.span.end.checked_sub(body.len()).ok_or("Invalid HLSL function body range")?;
        scopes.push((body_start..function.span.end, Some(function_index)));
    }
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if tokens[index] == "shader_body" && tokens[index + 1] == "{" {
            let end = close_brace(tokens, index + 1)?;
            scopes.push((index + 1..end, None));
            index = end;
        } else {
            index += 1;
        }
    }

    let mut edits = Vec::<(Range<usize>, Vec<String>)>::new();
    for (scope, function_index) in scopes {
        let mut depth = 0i32;
        let mut candidates = Vec::<(usize, usize, String, String, String)>::new();
        let mut index = scope.start;
        while index < scope.end {
            if tokens[index] == "{" { depth += 1; index += 1; continue; }
            if tokens[index] == "}" { depth -= 1; index += 1; continue; }
            if depth == 1 && resource_only.contains(&tokens[index])
                && tokens.get(index + 1).is_some_and(|name| is_identifier(name))
                && tokens.get(index + 2).map(String::as_str) == Some("=")
                && tokens.get(index + 3).is_some_and(|global| globals.contains_key(global))
                && tokens.get(index + 4).map(String::as_str) == Some(";") {
                let ty = tokens[index].clone();
                let name = tokens[index + 1].clone();
                let target = tokens[index + 3].clone();
                candidates.push((index, index + 5, ty, name, target));
                index += 5;
                continue;
            }
            index += 1;
        }
        if candidates.is_empty() { continue; }
        let mut counts = BTreeMap::<String, usize>::new();
        for (_, _, _, name, _) in &candidates { *counts.entry(name.clone()).or_default() += 1; }

        let parameter_names: BTreeSet<String> = function_index
            .and_then(|function_index| functions.get(function_index))
            .into_iter()
            .flat_map(|function| function.parameters.iter())
            .filter_map(|parameter| {
                let end = parameter.iter().position(|token| matches!(token.as_str(), ":" | "=" | "["))
                    .unwrap_or(parameter.len());
                parameter[..end].last().filter(|name| is_identifier(name)).cloned()
            })
            .collect();

        for (declaration_start, declaration_end, ty, name, target) in candidates {
            if counts.get(&name).copied() != Some(1)
                || globals.get(&target) != Some(&ty)
                || parameter_names.contains(&name)
                || parameter_names.contains(&target) {
                continue;
            }
            let has_shadow = (scope.start..scope.end).any(|index| {
                (index < declaration_start || index >= declaration_end)
                    && declaration_of_name(tokens, index, &name)
            });
            let target_shadowed = (scope.start..scope.end).any(|index| {
                declaration_of_name(tokens, index, &target)
            });
            let mut mutated = false;
            for index in scope.clone() {
                if (declaration_start..declaration_end).contains(&index)
                    || tokens[index] != name || (index > 0 && tokens[index - 1] == ".") {
                    continue;
                }
                let end = access_chain_end(tokens, index);
                if matches!(tokens.get(end).map(String::as_str), Some("=") | Some("+=") | Some("-=")
                    | Some("*=") | Some("/=") | Some("%=") | Some("&=") | Some("|=")
                    | Some("^=") | Some("<<=") | Some(">>=") | Some("++") | Some("--"))
                    || index > scope.start && matches!(tokens[index - 1].as_str(), "++" | "--") {
                    mutated = true;
                    break;
                }
            }
            if has_shadow || target_shadowed || mutated { continue; }

            edits.push((declaration_start..declaration_end, Vec::new()));
            for index in scope.clone() {
                if (declaration_start..declaration_end).contains(&index)
                    || tokens[index] != name || (index > 0 && tokens[index - 1] == ".") {
                    continue;
                }
                edits.push((index..index + 1, vec![target.clone()]));
            }
        }
    }

    edits.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in edits {
        tokens.splice(range, replacement);
    }
    Ok(())
}

fn opaque_name(parameter: &[String], resource_structs: &BTreeSet<String>) -> Result<Option<String>, String> {
    let first = parameter.iter().position(|t| !matches!(t.as_str(), "in" | "const" | "uniform" | "out" | "inout"));
    let Some(first) = first else { return Ok(None); };
    let opaque_type = matches!(parameter[first].as_str(), "SamplerState" | "SamplerComparisonState" | "Texture1D" | "Texture2D" | "Texture3D"
        | "sampler" | "sampler2D" | "sampler3D");
    let resource_struct = resource_structs.contains(parameter[first].as_str());
    if !opaque_type && !resource_struct { return Ok(None); }
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
    // Normalize opaque local values that can be resolved to named global bindings
    // before helper specialization and typed method lowering.
    normalize_static_local_resource_arrays(tokens)?;
    normalize_static_local_resource_aliases(tokens)?;
    // Preserve the existing typed opaque-parameter path (including overloads)
    // when no initialized state needs forwarding through a global alias.
    let resource_structs = resource_struct_types(tokens);
    let functions = declarations(tokens)?;
    let has_resource_struct_parameter = functions.iter().flat_map(|function| &function.parameters).any(|parameter| {
        let first = parameter.iter().position(|token| !matches!(token.as_str(), "in" | "const" | "uniform" | "out" | "inout"));
        first.and_then(|index| parameter.get(index)).is_some_and(|ty| resource_structs.contains(ty))
    });
    let has_static_state = has_resource_struct_parameter || tokens.iter().enumerate().any(|(index, token)| token == "sampler_state"
        || matches!(token.as_str(), "SamplerState" | "SamplerComparisonState")
            && matches!(tokens.get(index + 2).map(String::as_str), Some("{") | Some("=")));
    if !has_static_state { return Ok(()); }
    let mut templates = BTreeMap::<String, Vec<Template>>::new();
    for function in functions.iter().filter(|f| f.body.is_some()) {
        let opaque = function.parameters.iter().map(|p| opaque_name(p, &resource_structs)).collect::<Result<Vec<_>, _>>()?;
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
