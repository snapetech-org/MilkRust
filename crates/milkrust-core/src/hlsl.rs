//! MilkDrop shader environment for the typed legacy HLSL compiler.
use crate::milk2;
use std::collections::BTreeMap;

fn is_identifier(token: &str) -> bool {
    token.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && token.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[derive(Clone, Copy)]
struct ResourceShape {
    dimension: u8,
    width: u8,
}

#[derive(Clone, Copy)]
struct SamplerStateProfile {
    min_point: bool,
    mag_point: bool,
    mip: crate::resources::MipFilter,
    wrap: [crate::resources::AddressMode; 3],
}

const LEGACY_SAMPLER_INTRINSICS: &[&str] = &[
    "tex", "tex2d", "tex3d", "tex2D", "tex2Dlod", "tex2Dbias", "tex2Dgrad", "tex2Dproj", "tex2Doffset",
    "tex3D", "tex3Dlod", "tex3Dbias", "tex3Dgrad", "tex3Dproj", "tex3Doffset", "tex2Dfetch", "tex3Dfetch",
    "tex2Dsize", "tex3Dsize",
];

impl SamplerStateProfile {
    fn binding_name(self, resource: &str, dimension: u8) -> Result<String, String> {
        if !(1..=3).contains(&dimension) {
            return Err("HLSL sampler state has an invalid texture dimension".into());
        }
        let filter_code = usize::from(!self.min_point) * 6 + usize::from(!self.mag_point) * 3 + match self.mip {
            crate::resources::MipFilter::None => 0,
            crate::resources::MipFilter::Point => 1,
            crate::resources::MipFilter::Linear => 2,
        };
        let filter = if filter_code < 10 {
            char::from(b'0' + filter_code as u8)
        } else {
            char::from(b'a' + (filter_code - 10) as u8)
        };
        let address = self.wrap.iter().map(|mode| match mode {
            crate::resources::AddressMode::Wrap => 'w',
            crate::resources::AddressMode::Clamp => 'c',
            crate::resources::AddressMode::Mirror => 'm',
        }).collect::<String>();
        Ok(format!("sampler_hlsl_{filter}{address}_{resource}"))
    }
}

fn parse_sampler_state_profile(tokens: &[String], open: usize) -> Result<(SamplerStateProfile, usize), String> {
    if tokens.get(open).map(String::as_str) != Some("{") {
        return Err("Malformed HLSL SamplerState initializer".into());
    }
    let mut profile = SamplerStateProfile {
        min_point: false, mag_point: false, mip: crate::resources::MipFilter::Linear,
        wrap: [crate::resources::AddressMode::Wrap; 3],
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut cursor = open + 1;
    while cursor < tokens.len() && tokens[cursor] != "}" {
        let property = tokens[cursor].as_str();
        if !matches!(property, "Filter" | "AddressU" | "AddressV" | "AddressW") {
            return Err(format!("HLSL SamplerState property {property} is outside the point/linear wrap/clamp/mirror profile"));
        }
        if !seen.insert(property.to_string()) {
            return Err(format!("Duplicate HLSL SamplerState property {property}"));
        }
        if tokens.get(cursor + 1).map(String::as_str) != Some("=")
            || tokens.get(cursor + 3).map(String::as_str) != Some(";")
        {
            return Err(format!("Malformed HLSL SamplerState property {property}"));
        }
        let value = tokens[cursor + 2].to_ascii_uppercase();
        match property {
            "Filter" => match value.as_str() {
                "MIN_MAG_MIP_POINT" | "POINT" => {
                    profile.min_point = true;
                    profile.mag_point = true;
                    profile.mip = crate::resources::MipFilter::Point;
                }
                "MIN_MAG_POINT_MIP_LINEAR" => {
                    profile.min_point = true;
                    profile.mag_point = true;
                    profile.mip = crate::resources::MipFilter::Linear;
                }
                "MIN_POINT_MAG_LINEAR_MIP_POINT" => {
                    profile.min_point = true;
                    profile.mag_point = false;
                    profile.mip = crate::resources::MipFilter::Point;
                }
                "MIN_POINT_MAG_MIP_LINEAR" => {
                    profile.min_point = true;
                    profile.mag_point = false;
                    profile.mip = crate::resources::MipFilter::Linear;
                }
                "MIN_LINEAR_MAG_MIP_POINT" => {
                    profile.min_point = false;
                    profile.mag_point = true;
                    profile.mip = crate::resources::MipFilter::Point;
                }
                "MIN_LINEAR_MAG_POINT_MIP_LINEAR" => {
                    profile.min_point = false;
                    profile.mag_point = true;
                    profile.mip = crate::resources::MipFilter::Linear;
                }
                "MIN_MAG_LINEAR_MIP_POINT" => {
                    profile.min_point = false;
                    profile.mag_point = false;
                    profile.mip = crate::resources::MipFilter::Point;
                }
                "MIN_MAG_MIP_LINEAR" | "LINEAR" => {
                    profile.min_point = false;
                    profile.mag_point = false;
                    profile.mip = crate::resources::MipFilter::Linear;
                }
                _ => return Err(format!("HLSL SamplerState filter {value} is unsupported; use a standard point/linear filter combination")),
            },
            "AddressU" | "AddressV" | "AddressW" => {
                let wrap = match value.as_str() {
                    "WRAP" => crate::resources::AddressMode::Wrap,
                    "CLAMP" => crate::resources::AddressMode::Clamp,
                    "MIRROR" => crate::resources::AddressMode::Mirror,
                    _ => return Err(format!("HLSL SamplerState address mode {value} is unsupported; use Wrap, Clamp or Mirror")),
                };
                let axis = match property { "AddressU" => 0, "AddressV" => 1, _ => 2 };
                profile.wrap[axis] = wrap;
            }
            _ => unreachable!(),
        }
        cursor += 4;
    }
    if tokens.get(cursor).map(String::as_str) != Some("}") {
        return Err("Unterminated HLSL SamplerState initializer".into());
    }
    Ok((profile, cursor))
}

/// Parse the bounded subset of the legacy D3D effect-file `sampler_state`
/// initializer that can be represented by the renderer's sampler table.
/// Unspecified filter/address fields use MilkDrop's linear/repeat convention.
fn parse_legacy_sampler_state(
    tokens: &[String],
    open: usize,
    fallback_resource: Option<&str>,
) -> Result<(SamplerStateProfile, String, usize), String> {
    if tokens.get(open).map(String::as_str) != Some("{") {
        return Err("Malformed legacy HLSL sampler_state initializer".into());
    }
    let mut profile = SamplerStateProfile {
        min_point: false, mag_point: false, mip: crate::resources::MipFilter::Linear,
        wrap: [crate::resources::AddressMode::Wrap; 3],
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut resource = None;
    let mut cursor = open + 1;
    while cursor < tokens.len() && tokens[cursor] != "}" {
        let property = tokens[cursor].as_str();
        if !matches!(property, "Texture" | "MinFilter" | "MagFilter" | "MipFilter" | "AddressU" | "AddressV" | "AddressW") {
            return Err(format!("Legacy HLSL sampler_state property {property} is outside the texture/point/linear/wrap/clamp/mirror profile"));
        }
        if !seen.insert(property.to_string()) {
            return Err(format!("Duplicate legacy HLSL sampler_state property {property}"));
        }
        if tokens.get(cursor + 1).map(String::as_str) != Some("=") {
            return Err(format!("Malformed legacy HLSL sampler_state property {property}"));
        }
        if property == "Texture" {
            if tokens.get(cursor + 2).map(String::as_str) != Some("<")
                || !tokens.get(cursor + 3).is_some_and(|name| is_identifier(name))
                || tokens.get(cursor + 4).map(String::as_str) != Some(">")
                || tokens.get(cursor + 5).map(String::as_str) != Some(";")
            {
                return Err("Legacy HLSL sampler_state Texture must be a single <identifier> binding".into());
            }
            resource = Some(crate::resources::base_name(&tokens[cursor + 3]).to_string());
            cursor += 6;
            continue;
        }
        if tokens.get(cursor + 3).map(String::as_str) != Some(";") {
            return Err(format!("Malformed legacy HLSL sampler_state property {property}"));
        }
        let value = tokens[cursor + 2].to_ascii_uppercase();
        match property {
            "MinFilter" => {
                let point = match value.as_str() {
                    "POINT" => true,
                    "LINEAR" => false,
                    _ => return Err(format!("Legacy HLSL sampler_state min filter {value} is unsupported; use Point or Linear")),
                };
                profile.min_point = point;
            }
            "MagFilter" => {
                let point = match value.as_str() {
                    "POINT" => true,
                    "LINEAR" => false,
                    _ => return Err(format!("Legacy HLSL sampler_state mag filter {value} is unsupported; use Point or Linear")),
                };
                profile.mag_point = point;
            }
            "MipFilter" => {
                profile.mip = match value.as_str() {
                    "NONE" => crate::resources::MipFilter::None,
                    "POINT" => crate::resources::MipFilter::Point,
                    "LINEAR" => crate::resources::MipFilter::Linear,
                    _ => return Err(format!("Legacy HLSL sampler_state mip filter {value} is unsupported; use None, Point or Linear")),
                };
            }
            "AddressU" | "AddressV" | "AddressW" => {
                let wrap = match value.as_str() {
                    "WRAP" => crate::resources::AddressMode::Wrap,
                    "CLAMP" => crate::resources::AddressMode::Clamp,
                    "MIRROR" => crate::resources::AddressMode::Mirror,
                    _ => return Err(format!("Legacy HLSL sampler_state address mode {value} is unsupported; use Wrap, Clamp or Mirror")),
                };
                let axis = match property { "AddressU" => 0, "AddressV" => 1, _ => 2 };
                profile.wrap[axis] = wrap;
            }
            _ => unreachable!(),
        }
        cursor += 4;
    }
    if tokens.get(cursor).map(String::as_str) != Some("}") {
        return Err("Unterminated legacy HLSL sampler_state initializer".into());
    }
    let resource = resource.or_else(|| fallback_resource.map(str::to_string))
        .ok_or_else(|| "Legacy HLSL sampler_state requires Texture = <name> unless the sampler uses a sampler_* binding name".to_string())?;
    if !is_identifier(&resource) {
        return Err("Legacy HLSL sampler_state Texture must resolve to an identifier binding".into());
    }
    Ok((profile, resource, cursor))
}

fn texture_element_width(tokens: &[String]) -> Result<u8, String> {
    if tokens.len() != 1 { return Err("Unsupported HLSL texture element type".into()); }
    for prefix in ["float", "half", "min16float", "min10float"] {
        if let Some(width) = tokens[0].strip_prefix(prefix) {
            if width.is_empty() { return Ok(1); }
            if let Ok(width) = width.parse::<u8>() {
                if (1..=4).contains(&width) { return Ok(width); }
            }
        }
    }
    Err("MilkRust texture objects require float, half, or minimum-precision float scalar/vector color elements".into())
}

fn hlsl_struct_ranges(tokens: &[String]) -> Result<Vec<(String, std::ops::Range<usize>)>, String> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index + 2 < tokens.len() {
        if tokens[index] != "struct" { index += 1; continue; }
        let name = tokens[index + 1].clone();
        let Some(open) = tokens[index + 2..].iter().position(|token| token == "{").map(|offset| index + 2 + offset) else {
            return Err("Malformed HLSL struct declaration".into());
        };
        let mut depth = 1i32;
        let mut end = open + 1;
        while end < tokens.len() && depth > 0 {
            if tokens[end] == "{" { depth += 1; }
            if tokens[end] == "}" { depth -= 1; }
            end += 1;
        }
        if depth != 0 { return Err("Unterminated HLSL struct declaration".into()); }
        ranges.push((name, index..end));
        index = end;
    }
    Ok(ranges)
}

fn split_hlsl_call(tokens: &[String], open: usize) -> Result<(Vec<Vec<String>>, usize), String> {
    if tokens.get(open).map(String::as_str) != Some("(") {
        return Err("Malformed HLSL resource method call".into());
    }
    let mut parens = 1i32;
    let mut brackets = 0i32;
    let mut braces = 0i32;
    let mut start = open + 1;
    let mut args = Vec::new();
    let mut index = open + 1;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "(" => parens += 1,
            ")" => {
                parens -= 1;
                if parens == 0 {
                    if index > start || !args.is_empty() { args.push(tokens[start..index].to_vec()); }
                    return Ok((args, index + 1));
                }
            }
            "[" => brackets += 1,
            "]" => brackets -= 1,
            "{" => braces += 1,
            "}" => braces -= 1,
            "," if parens == 1 && brackets == 0 && braces == 0 => {
                args.push(tokens[start..index].to_vec());
                start = index + 1;
            }
            _ => {}
        }
        if parens < 1 || brackets < 0 || braces < 0 { return Err("Unbalanced HLSL resource method arguments".into()); }
        index += 1;
    }
    Err("Unterminated HLSL resource method call".into())
}

fn matching_hlsl_bracket(tokens: &[String], open: usize) -> Result<usize, String> {
    if tokens.get(open).map(String::as_str) != Some("[") {
        return Err("Malformed HLSL resource-array access".into());
    }
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.as_str() {
            "[" => depth += 1,
            "]" => {
                depth -= 1;
                if depth == 0 { return Ok(index); }
            }
            _ => {}
        }
    }
    Err("Unterminated HLSL resource-array access".into())
}

fn repeatable_resource_index(tokens: &[String]) -> bool {
    !tokens.is_empty()
        && !tokens.windows(2).any(|pair| is_identifier(&pair[0]) && pair[1] == "(")
        && tokens.iter().all(|token| {
            is_identifier(token)
                || token.as_bytes().first().is_some_and(|byte| byte.is_ascii_digit())
                || matches!(token.as_str(), "(" | ")" | "+" | "-" | "*" | "/" | "%" | "<<" | ">>"
                    | "&" | "|" | "^" | "~" | "!" | "<" | ">" | "<=" | ">=" | "==" | "!=" | "&&" | "||")
        })
}

fn grouped_resource_declarators(tokens: &[String], first: usize) -> Result<(Vec<(String, Option<usize>)>, usize), String> {
    let mut declarators = Vec::new();
    let mut cursor = first;
    loop {
        let Some(name) = tokens.get(cursor).filter(|name| is_identifier(name)) else {
            return Err("Malformed HLSL resource declaration".into());
        };
        let name = name.clone();
        cursor += 1;
        let size = if tokens.get(cursor).map(String::as_str) == Some("[") {
            let size = tokens.get(cursor + 1).and_then(|token| token.parse::<usize>().ok())
                .filter(|size| (1..=15).contains(size))
                .ok_or_else(|| "HLSL resource arrays require a decimal literal bound from 1 through 15".to_string())?;
            if tokens.get(cursor + 2).map(String::as_str) != Some("]") {
                return Err("HLSL resource array bounds must be a single decimal literal".into());
            }
            cursor += 3;
            Some(size)
        } else {
            None
        };
        if tokens.get(cursor).map(String::as_str) == Some("=") {
            return Err("Initialized HLSL texture objects are unsupported".into());
        }
        declarators.push((name, size));
        if tokens.get(cursor).map(String::as_str) != Some(",") { break; }
        let next = cursor + 1;
        if !tokens.get(next).is_some_and(|name| is_identifier(name)) { break; }
        let following = tokens.get(next + 1).map(String::as_str);
        if !matches!(following, Some("[") | Some("=") | Some(",") | Some(";") ) {
            // A typed function parameter follows the comma. It is not a
            // grouped declarator in the current resource declaration.
            break;
        }
        cursor = next;
    }
    Ok((declarators, cursor))
}

fn resource_declaration_has_array(tokens: &[String], first: usize) -> bool {
    let mut cursor = first;
    loop {
        if !tokens.get(cursor).is_some_and(|name| is_identifier(name)) { return false; }
        cursor += 1;
        if tokens.get(cursor).map(String::as_str) == Some("[") { return true; }
        if tokens.get(cursor).map(String::as_str) != Some(",") { return false; }
        cursor += 1;
        if !tokens.get(cursor).is_some_and(|name| is_identifier(name)) { return false; }
        if tokens.get(cursor + 1).map(String::as_str) == Some("[") { return true; }
        if !matches!(tokens.get(cursor + 1).map(String::as_str), Some(",") | Some(";")) { return false; }
    }
}

fn splice_bounded_resource_expansion(
    tokens: &mut Vec<String>,
    range: std::ops::Range<usize>,
    replacement: Vec<String>,
    expanded_bytes: &mut usize,
) -> Result<(), String> {
    let original_bytes = tokens[range.clone()].iter().map(String::len).sum::<usize>();
    let replacement_bytes = replacement.iter().map(String::len).sum::<usize>();
    *expanded_bytes = (*expanded_bytes).saturating_add(replacement_bytes.saturating_sub(original_bytes));
    if *expanded_bytes > 512 * 1024 {
        return Err("HLSL resource-array dispatch exceeds its 512 KiB expansion budget".into());
    }
    tokens.splice(range, replacement);
    Ok(())
}

/// Lower HLSL resource objects into the legacy sampler intrinsics already
/// represented in the typed AST. Bounded global 1D/2D/3D texture and legacy
/// sampler arrays expand into named bindings; constant and side-effect-free
/// dynamic indices dispatch over those names. Static global point/linear
/// SamplerState definitions and bounded legacy sampler_state initializers lower
/// through per-resource aliases.
/// Function-local arrays, nested resource structs and advanced sampler states
/// need different host binding models and fail explicitly.
fn lower_modern_resource_syntax(tokens: &mut Vec<String>) -> Result<(), String> {
    let struct_ranges = hlsl_struct_ranges(tokens)?;
    let mut resource_shapes = BTreeMap::<String, Option<ResourceShape>>::new();
    let mut resource_arrays = BTreeMap::<String, usize>::new();
    let mut resource_array_edits = Vec::<(std::ops::Range<usize>, Vec<String>)>::new();
    let mut sampler_state_names = std::collections::BTreeSet::new();
    let mut sampler_state_profiles = BTreeMap::<String, SamplerStateProfile>::new();
    let mut sampler_state_fields = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    let mut legacy_sampler_bindings = BTreeMap::<String, String>::new();
    let mut generic_ranges = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let dimension = match tokens[index].as_str() {
            "Texture1D" => Some(1),
            "Texture2D" => Some(2),
            "Texture3D" => Some(3),
            "Texture1DArray" | "Texture2DArray" | "Texture2DMS" | "Texture2DMSArray"
            | "TextureCube" | "TextureCubeArray" | "SamplerComparisonState" | "RWTexture1D"
            | "RWTexture1DArray" | "RWTexture2D" | "RWTexture2DArray" | "RWTexture3D"
            | "RasterizerOrderedTexture1D" | "RasterizerOrderedTexture2D" | "RasterizerOrderedTexture3D"
            | "Buffer" | "RWBuffer" | "StructuredBuffer" | "RWStructuredBuffer"
            | "AppendStructuredBuffer" | "ConsumeStructuredBuffer" | "ByteAddressBuffer"
            | "RWByteAddressBuffer" | "ConstantBuffer" => {
                return Err(format!("{} is outside the 1D/2D/3D MilkRust texture profile", tokens[index]));
            }
            _ => None,
        };
        if let Some(dimension) = dimension {
            let type_index = index;
            let after_type = index + 1;
            let mut generic_end = None;
            let mut generic_range = None;
            let mut width = 4;
            if tokens.get(after_type).map(String::as_str) == Some("<") {
                let Some(relative_end) = tokens[after_type + 1..].iter().position(|token| token == ">") else {
                    return Err("Unterminated HLSL texture element type".into());
                };
                let end = after_type + 1 + relative_end;
                if end == after_type + 1 || tokens[after_type + 1..end].iter().any(|token| matches!(token.as_str(), "<" | ";" | "{")) {
                    return Err("Unsupported HLSL texture element type".into());
                }
                width = texture_element_width(&tokens[after_type + 1..end])?;
                generic_range = Some(after_type..end + 1);
                generic_end = Some(end);
            }
            let name_index = generic_end.map_or(after_type, |end| end + 1);
            if tokens.get(name_index).is_some_and(|token| token == "(") {
                return Err("Texture objects cannot be returned from HLSL functions".into());
            }
            let Some(name) = tokens.get(name_index).filter(|token| is_identifier(token)) else {
                return Err("Malformed HLSL texture object declaration".into());
            };
            if tokens.get(name_index + 1).map(String::as_str) == Some("=") {
                return Err("Initialized HLSL texture objects are unsupported".into());
            }
            let structure = struct_ranges.iter().find(|(_, range)| range.contains(&type_index)).map(|(name, _)| name.as_str());
            let shape = ResourceShape { dimension, width };
            if resource_declaration_has_array(tokens, name_index) {
                if structure.is_some() {
                    if struct_ranges.iter().filter(|(_, range)| range.contains(&type_index)).count() != 1 {
                        return Err("Nested HLSL resource-array structs are unsupported".into());
                    }
                } else {
                    let mut parens = 0i32;
                    let mut braces = 0i32;
                    for token in &tokens[..type_index] {
                        if token == "(" { parens += 1; }
                        if token == ")" { parens -= 1; }
                        if token == "{" { braces += 1; }
                        if token == "}" { braces -= 1; }
                    }
                    if parens != 0 || braces != 0 {
                        return Err("Texture resource arrays are supported only as global declarations".into());
                    }
                }
                let (declarators, declaration_end) = grouped_resource_declarators(tokens, name_index)?;
                if !declarators.iter().any(|(_, size)| size.is_some()) {
                    return Err("Malformed HLSL texture-array declaration".into());
                }
                let mut declaration = vec![if dimension <= 2 { "sampler2D" } else { "sampler3D" }.to_string()];
                let mut emitted = 0usize;
                for (declarator, size) in declarators {
                    if let Some(size) = size {
                        let array_name = structure.map_or_else(|| declarator.clone(), |structure| format!("{structure}.{declarator}"));
                        resource_arrays.insert(array_name, size);
                        for element in 0..size {
                            let element_name = format!("{}_{}", declarator, element);
                            if tokens.iter().any(|token| token == &element_name) {
                                return Err(format!("Generated HLSL texture-array binding {element_name} collides with a user identifier"));
                            }
                            if emitted != 0 { declaration.push(",".into()); }
                            declaration.push(element_name.clone());
                            emitted += 1;
                            let shape_name = structure.map_or_else(|| element_name.clone(), |structure| format!("{structure}.{element_name}"));
                            resource_shapes.insert(shape_name, Some(shape));
                        }
                    } else {
                        if emitted != 0 { declaration.push(",".into()); }
                        declaration.push(declarator.clone());
                        emitted += 1;
                        let shape_name = structure.map_or_else(|| declarator.clone(), |structure| format!("{structure}.{declarator}"));
                        resource_shapes.insert(shape_name, Some(shape));
                    }
                }
                resource_array_edits.push((type_index..declaration_end, declaration));
                index = declaration_end;
                continue;
            }
            if let Some(range) = generic_range { generic_ranges.push(range); }
            let mut declarators = vec![name.clone()];
            let mut declarator = name_index + 1;
            while tokens.get(declarator).map(String::as_str) == Some(",") {
                let candidate = declarator + 1;
                let next = tokens.get(candidate + 1).map(String::as_str);
                if !tokens.get(candidate).is_some_and(|token| is_identifier(token)) { break; }
                if next == Some("[") { return Err("Grouped HLSL texture-array declarations are unsupported".into()); }
                if next == Some("=") { return Err("Initialized HLSL texture objects are unsupported".into()); }
                // A grouped declaration has `name, name[, ...];`. A function
                // signature's comma is followed by a type and a parameter.
                if !matches!(next, Some(",") | Some(";")) { break; }
                declarators.push(tokens[candidate].clone());
                declarator = candidate + 1;
            }
            for name in declarators {
                let key = structure.map_or_else(|| name.clone(), |structure| format!("{structure}.{name}"));
                resource_shapes.entry(key).and_modify(|previous| {
                    if previous.as_ref().is_some_and(|previous| previous.dimension != shape.dimension || previous.width != shape.width) {
                        *previous = None;
                    }
                }).or_insert(Some(shape));
            }
            tokens[type_index] = if dimension <= 2 { "sampler2D" } else { "sampler3D" }.into();
            index = name_index + 1;
            continue;
        }
        if matches!(tokens[index].as_str(), "sampler" | "sampler2D" | "sampler3D")
            && tokens.get(index + 1).is_some_and(|name| is_identifier(name))
            && resource_declaration_has_array(tokens, index + 1) {
            let type_index = index;
            let name_index = index + 1;
            let structure = struct_ranges.iter().find(|(_, range)| range.contains(&type_index)).map(|(name, _)| name.as_str());
            if structure.is_some() {
                if struct_ranges.iter().filter(|(_, range)| range.contains(&type_index)).count() != 1 {
                    return Err("Nested HLSL resource-array structs are unsupported".into());
                }
            } else {
                let mut parens = 0i32;
                let mut braces = 0i32;
                for token in &tokens[..type_index] {
                    if token == "(" { parens += 1; }
                    if token == ")" { parens -= 1; }
                    if token == "{" { braces += 1; }
                    if token == "}" { braces -= 1; }
                }
                if parens != 0 || braces != 0 {
                    return Err("Sampler arrays are supported only as global declarations".into());
                }
            }
            let (declarators, declaration_end) = grouped_resource_declarators(tokens, name_index)?;
            if !declarators.iter().any(|(_, size)| size.is_some()) {
                return Err("Malformed HLSL sampler-array declaration".into());
            }
            let mut declaration = vec![tokens[type_index].clone()];
            let mut emitted = 0usize;
            for (name, size) in declarators {
                if let Some(size) = size {
                    let array_name = structure.map_or_else(|| name.clone(), |structure| format!("{structure}.{name}"));
                    resource_arrays.insert(array_name, size);
                    for element in 0..size {
                        let element_name = format!("{}_{}", name, element);
                        if tokens.iter().any(|token| token == &element_name) {
                            return Err(format!("Generated HLSL sampler-array binding {element_name} collides with a user identifier"));
                        }
                        if emitted != 0 { declaration.push(",".into()); }
                        declaration.push(element_name);
                        emitted += 1;
                    }
                } else {
                    if emitted != 0 { declaration.push(",".into()); }
                    declaration.push(name);
                    emitted += 1;
                }
            }
            resource_array_edits.push((type_index..declaration_end, declaration));
            index = declaration_end;
            continue;
        }
        if matches!(tokens[index].as_str(), "sampler" | "sampler2D" | "sampler3D")
            && tokens.get(index + 1).is_some_and(|name| is_identifier(name)) {
            let type_index = index;
            let name_index = index + 1;
            let sampler_name = tokens[name_index].clone();
            let mut initializer = None;
            let mut cursor = name_index + 1;
            let mut parens = 0i32;
            while cursor < tokens.len() {
                match tokens[cursor].as_str() {
                    "(" => parens += 1,
                    ")" => parens -= 1,
                    "=" if parens == 0 => { initializer = Some(cursor); break; }
                    ";" | "," if parens == 0 => break,
                    _ => {}
                }
                cursor += 1;
            }
            if let Some(equals) = initializer {
                if tokens.get(equals + 1).map(String::as_str) != Some("sampler_state")
                    || tokens.get(equals + 2).map(String::as_str) != Some("{")
                {
                    return Err("Initialized legacy HLSL samplers require a static sampler_state block".into());
                }
                let global_scope = tokens[..type_index].iter().fold(0i32, |depth, token| match token.as_str() {
                    "{" => depth + 1,
                    "}" => depth - 1,
                    _ => depth,
                }) == 0;
                if !global_scope {
                    return Err("Initialized legacy HLSL samplers must be global declarations".into());
                }
                let fallback_resource = sampler_name.starts_with("sampler_")
                    .then(|| crate::resources::base_name(&sampler_name))
                    .filter(|resource| is_identifier(resource));
                let (profile, resource, close) = parse_legacy_sampler_state(tokens, equals + 2, fallback_resource)?;
                let semicolon = close + 1;
                if tokens.get(semicolon).map(String::as_str) != Some(";") {
                    return Err("Grouped legacy HLSL sampler_state declarations are unsupported".into());
                }
                let alias = profile.binding_name(&resource, if tokens[type_index] == "sampler3D" { 3 } else { 2 })?;
                if tokens.iter().any(|token| token == &alias) {
                    return Err(format!("Generated HLSL sampler binding {alias} collides with a user identifier"));
                }
                if legacy_sampler_bindings.insert(sampler_name.clone(), alias).is_some() {
                    return Err(format!("Duplicate initialized legacy HLSL sampler {sampler_name}"));
                }
                let edit_start = if type_index > 0 && tokens[type_index - 1] == "uniform" { type_index - 1 } else { type_index };
                resource_array_edits.push((edit_start..semicolon + 1, Vec::new()));
                index = semicolon + 1;
                continue;
            }
        }
        if tokens[index] == "SamplerState" {
            let Some(name) = tokens.get(index + 1).filter(|token| is_identifier(token)) else {
                return Err("Malformed HLSL SamplerState declaration".into());
            };
            // Sampling state is selected by the MilkDrop sampler naming
            // convention on both hosts. Supported static state initializers
            // are converted into the same explicit sampler prefixes.
            let structure = struct_ranges.iter().find(|(_, range)| range.contains(&index)).map(|(name, _)| name.clone());
            let mut initializer = None;
            match tokens.get(index + 2).map(String::as_str) {
                Some("[") => return Err("HLSL SamplerState arrays are unsupported".into()),
                Some("{") => initializer = Some((index + 2, index + 2)),
                Some("=") => match tokens.get(index + 3).map(String::as_str) {
                    Some("{") => initializer = Some((index + 2, index + 3)),
                    Some("sampler_state") if tokens.get(index + 4).map(String::as_str) == Some("{") => initializer = Some((index + 2, index + 4)),
                    _ => return Err("Dynamic or unsupported HLSL SamplerState initializers are unavailable".into()),
                },
                _ => {}
            }
            if let Some((edit_start, open)) = initializer {
                if structure.is_some() || tokens[..index].iter().fold(0i32, |depth, token| match token.as_str() {
                    "{" => depth + 1,
                    "}" => depth - 1,
                    _ => depth,
                }) != 0 {
                    return Err("Initialized HLSL SamplerState declarations must be global".into());
                }
                let (profile, close) = parse_sampler_state_profile(tokens, open)?;
                if !matches!(tokens.get(close + 1).map(String::as_str), Some(";") | None) {
                    return Err("Grouped HLSL SamplerState initializers are unsupported".into());
                }
                sampler_state_profiles.insert(name.clone(), profile);
                resource_array_edits.push((edit_start..close + 1, Vec::new()));
            }
            if let Some(structure) = &structure {
                sampler_state_fields.entry(structure.clone()).or_default().insert(name.clone());
            } else {
                sampler_state_names.insert(name.clone());
            }
            let declarator = if initializer.is_some() { tokens.len() } else { index + 2 };
            while tokens.get(declarator).map(String::as_str) == Some(",") {
                let candidate = declarator + 1;
                let next = tokens.get(candidate + 1).map(String::as_str);
                if !tokens.get(candidate).is_some_and(|token| is_identifier(token)) { break; }
                if matches!(next, Some("[") | Some("=") | Some("{")) {
                    return Err("SamplerState arrays and grouped initializers are unsupported; declare each supported global state separately".into());
                }
                if !matches!(next, Some(",") | Some(";") | Some(")") | Some(":")) {
                    break;
                }
                if let Some(structure) = &structure {
                    sampler_state_fields.entry(structure.clone()).or_default().insert(tokens[candidate].clone());
                } else {
                    sampler_state_names.insert(tokens[candidate].clone());
                }
                declarator = candidate + 1;
            }
            tokens[index] = "int".into();
        }
        index += 1;
    }

    // The lowered resource methods translate supported state arguments into
    // MilkDrop sampler prefixes. Resolve declared struct instances and
    // parameters so a same-named numeric member is never treated as a state.
    let mut sampler_state_instances = BTreeMap::<String, Option<String>>::new();
    let record_sampler_state_instance = |instances: &mut BTreeMap<String, Option<String>>, name: &str, structure: &str| {
        instances.entry(name.to_string()).and_modify(|previous| {
            if previous.as_deref().is_some_and(|previous| previous != structure) { *previous = None; }
        }).or_insert_with(|| Some(structure.to_string()));
    };
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if let Some((structure, range)) = struct_ranges.iter().find(|(_, range)| range.start == cursor) {
            // Also handle the legal `struct S { ... } instance;` form where
            // the variable declarator immediately follows the closing brace.
            let mut declaration = range.end;
            if sampler_state_fields.contains_key(structure)
                && tokens.get(declaration).is_some_and(|name| is_identifier(name)) {
                loop {
                    let Some(name) = tokens.get(declaration).filter(|name| is_identifier(name)) else { break; };
                    record_sampler_state_instance(&mut sampler_state_instances, name, structure);
                    match tokens.get(declaration + 1).map(String::as_str) {
                        Some(",") => declaration += 2,
                        Some(";") => break,
                        _ => break,
                    }
                }
            }
            cursor = range.end;
            continue;
        }
        if sampler_state_fields.contains_key(&tokens[cursor])
            && tokens.get(cursor + 1).is_some_and(|name| is_identifier(name))
            && matches!(tokens.get(cursor + 2).map(String::as_str), Some(";") | Some(",") | Some(")") | Some(":") | Some("=")) {
            let structure = tokens[cursor].clone();
            let mut declaration = cursor + 1;
            loop {
                let Some(name) = tokens.get(declaration).filter(|name| is_identifier(name)) else { break; };
                record_sampler_state_instance(&mut sampler_state_instances, name, &structure);
                match tokens.get(declaration + 1).map(String::as_str) {
                    Some(",") => declaration += 2,
                    Some(";") => break,
                    _ => break,
                }
            }
        }
        cursor += 1;
    }
    let mut edits: Vec<(std::ops::Range<usize>, Vec<String>)> = generic_ranges.into_iter()
        .map(|range| (range, Vec::new())).chain(resource_array_edits).collect();
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in edits { tokens.splice(range, replacement); }
    let (_, _, resource_instances) = resource_struct_fields(tokens)?;

    // ESSL 3.00 restricts indexing of opaque sampler arrays. Expand resource
    // array references to separately named bindings. Dynamic accesses dispatch
    // over those bindings so the emitted shader never indexes an opaque value.
    let mut index = 0;
    let mut expanded_resource_array_bytes = 0usize;
    while index < tokens.len() {
        let scoped = if tokens.get(index + 1).map(String::as_str) == Some(".")
            && tokens.get(index + 3).map(String::as_str) == Some("[") {
            resource_instances.get(&tokens[index]).map(|structure| format!("{structure}.{}", tokens[index + 2]))
                .filter(|name| resource_arrays.contains_key(name))
        } else { None };
        let (array_name, bracket_index, replacement_start, scoped_structure) = if let Some(name) = scoped {
            let structure = resource_instances.get(&tokens[index]).cloned();
            (name, index + 3, index + 2, structure)
        } else if resource_arrays.contains_key(&tokens[index])
            && tokens.get(index + 1).map(String::as_str) == Some("[") {
            (tokens[index].clone(), index + 1, index, None)
        } else { index += 1; continue; };
        let size = resource_arrays.get(&array_name).copied().unwrap_or(0);
        let close_index = matching_hlsl_bracket(tokens, bracket_index)?;
        let array_index = tokens[bracket_index + 1..close_index].to_vec();
        let element = (array_index.len() == 1).then(|| array_index[0].parse::<usize>().ok()).flatten();
        if let Some(element) = element {
            if element >= size { return Err("HLSL texture resource array index is out of bounds".into()); }
            let element_name = format!("{}_{}", tokens[replacement_start], element);
            let shape_name = scoped_structure.as_ref().map_or_else(|| element_name.clone(), |structure| format!("{structure}.{element_name}"));
            if resource_shapes.contains_key(&shape_name) {
                let method = tokens.get(close_index + 2).map(String::as_str).unwrap_or("");
                if tokens.get(close_index + 1).map(String::as_str) != Some(".")
                    || !matches!(method, "Sample" | "SampleLevel" | "SampleBias" | "SampleGrad" | "Load" | "GetDimensions") {
                    return Err("HLSL texture array elements must use a supported resource method".into());
                }
            }
            tokens.splice(replacement_start..close_index + 1, [element_name]);
            index = replacement_start + 1;
            continue;
        }
        if !repeatable_resource_index(&array_index) {
            return Err("HLSL resource-array indices must be side-effect-free integer expressions".into());
        }

        let first_element = format!("{}_0", tokens[replacement_start]);
        let first_shape = scoped_structure.as_ref().map_or_else(|| first_element.clone(), |structure| format!("{structure}.{first_element}"));
        if resource_shapes.contains_key(&first_shape) {
            let shape = resource_shapes.get(&first_shape).copied().flatten()
                .ok_or_else(|| "Ambiguous HLSL resource-array element type".to_string())?;
            let method_index = close_index + 2;
            let method = tokens.get(method_index).map(String::as_str).unwrap_or("");
            if tokens.get(close_index + 1).map(String::as_str) != Some(".")
                || !matches!(method, "Sample" | "SampleLevel" | "SampleBias" | "SampleGrad" | "Load" | "GetDimensions")
                || tokens.get(method_index + 1).map(String::as_str) != Some("(") {
                return Err("Dynamically indexed HLSL texture arrays must immediately call a supported texture method".into());
            }
            let (args, call_end) = split_hlsl_call(tokens, method_index + 1)?;
            if args.iter().any(Vec::is_empty) {
                return Err(format!("Empty argument to HLSL texture method {method}"));
            }
            let receiver_prefix = tokens[index..replacement_start].to_vec();
            let array_base = tokens[replacement_start].clone();
            let receiver_name = |element: usize| {
                let mut receiver = receiver_prefix.clone();
                receiver.push(format!("{array_base}_{element}"));
                receiver
            };
            if method == "GetDimensions" {
                let dimensions = usize::from(shape.dimension);
                let has_mip = args.len() == dimensions + 2;
                if (args.len() != dimensions && !has_mip) || tokens.get(call_end).map(String::as_str) != Some(";") {
                    return Err("Dynamically indexed GetDimensions must be a standalone statement with valid output arguments".into());
                }
                let mut suffix = 0usize;
                let temporary = loop {
                    let candidate = format!("milkRustResourceArrayIndex{suffix}");
                    if !tokens.iter().any(|token| token == &candidate) { break candidate; }
                    suffix = suffix.saturating_add(1);
                };
                let outputs = if has_mip { args.iter().skip(1).collect::<Vec<_>>() } else { args.iter().collect::<Vec<_>>() };
                let mut replacement = vec!["{".into(), "int".into(), temporary.clone(), "=".into(), "int".into(), "(".into()];
                replacement.extend(array_index.clone());
                replacement.extend([")".into(), ";".into()]);
                for element in 0..size {
                    if element == 0 { replacement.push("if".into()); }
                    else { replacement.extend(["else".into(), "if".into()]); }
                    replacement.extend(["(".into(), temporary.clone(), "==".into(), element.to_string(), ")".into(), "{".into()]);
                    replacement.extend(receiver_name(element));
                    replacement.extend([".".into(), method.into(), "(".into()]);
                    for (argument_index, argument) in args.iter().enumerate() {
                        if argument_index != 0 { replacement.push(",".into()); }
                        replacement.extend(argument.iter().cloned());
                    }
                    replacement.extend([")".into(), ";".into(), "}".into()]);
                }
                replacement.extend(["else".into(), "{".into()]);
                for output in outputs {
                    replacement.extend_from_slice(output.as_slice());
                    replacement.extend(["=".into(), "0".into(), ";".into()]);
                }
                replacement.extend(["}".into(), "}".into()]);
                splice_bounded_resource_expansion(tokens, index..call_end + 1, replacement, &mut expanded_resource_array_bytes)?;
                index += 1;
                continue;
            }

            let width = shape.width;
            let mut replacement = vec!["(".into()];
            for element in 0..size {
                replacement.extend(["int".into(), "(".into()]);
                replacement.extend(array_index.clone());
                replacement.extend([")".into(), "==".into(), element.to_string(), "?".into()]);
                replacement.extend(receiver_name(element));
                replacement.extend([".".into(), method.into(), "(".into()]);
                for (argument_index, argument) in args.iter().enumerate() {
                    if argument_index != 0 { replacement.push(",".into()); }
                    replacement.extend(argument.iter().cloned());
                }
                replacement.extend([")".into(), ":".into()]);
                if element + 1 < size { replacement.push("(".into()); }
            }
            if width == 1 {
                replacement.push("0.0".into());
            } else {
                replacement.extend([format!("float{width}"), "(".into(), "0.0".into(), ")".into()]);
            }
            replacement.extend(std::iter::repeat(")".to_string()).take(size));
            splice_bounded_resource_expansion(tokens, index..call_end, replacement, &mut expanded_resource_array_bytes)?;
            index += 1;
            continue;
        }

        // Legacy sampler arrays are commonly indexed in the first parameter
        // of a tex* intrinsic. Duplicate the complete call per element rather
        // than attempting to select an opaque sampler with a GLSL expression.
        let open = index.checked_sub(1).filter(|open| tokens.get(*open).map(String::as_str) == Some("("));
        let Some(open) = open else {
            return Err("Dynamically indexed legacy sampler arrays must be the first argument to a supported tex* intrinsic".into());
        };
        let Some(intrinsic_index) = open.checked_sub(1) else {
            return Err("Malformed dynamically indexed legacy sampler call".into());
        };
        let intrinsic = tokens[intrinsic_index].as_str();
        if !matches!(intrinsic, "tex" | "tex2d" | "tex3d" | "tex2D" | "tex2Dlod" | "tex2Dbias" | "tex2Dgrad" | "tex2Dproj" | "tex2Doffset" | "tex3D" | "tex3Dlod" | "tex3Dbias" | "tex3Dgrad" | "tex3Dproj" | "tex3Doffset" | "tex2Dfetch") {
            return Err("Dynamically indexed legacy sampler arrays must be the first argument to a supported tex* intrinsic".into());
        }
        let (args, call_end) = split_hlsl_call(tokens, open)?;
        if args.is_empty() || args[0] != tokens[index..close_index + 1] || args.iter().any(Vec::is_empty) {
            return Err("Dynamically indexed legacy sampler-array access must occupy the complete first tex* argument".into());
        }
        let mut replacement = vec!["(".into()];
        for element in 0..size {
            replacement.extend(["int".into(), "(".into()]);
            replacement.extend(array_index.clone());
            replacement.extend([")".into(), "==".into(), element.to_string(), "?".into(), intrinsic.into(), "(".into()]);
            replacement.extend(receiver_name(element));
            for argument in args.iter().skip(1) {
                replacement.push(",".into());
                replacement.extend(argument.iter().cloned());
            }
            replacement.extend([")".into(), ":".into()]);
            if element + 1 < size { replacement.push("(".into()); }
        }
        replacement.extend(["float4".into(), "(".into(), "0.0".into(), ")".into()]);
        replacement.extend(std::iter::repeat(")".to_string()).take(size));
        splice_bounded_resource_expansion(tokens, intrinsic_index..call_end, replacement, &mut expanded_resource_array_bytes)?;
        index = intrinsic_index + 1;
    }
    let (resource_fields, _, resource_instances) = resource_struct_fields(tokens)?;

    let mut rewritten = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] != "." || index + 2 >= tokens.len() || tokens[index + 2] != "(" {
            rewritten.push(tokens[index].clone());
            index += 1;
            continue;
        }
        let method = tokens[index + 1].as_str();
        if !matches!(method, "Sample" | "SampleLevel" | "SampleBias" | "SampleGrad" | "Load" | "GetDimensions") {
            if index > 0 && is_identifier(&tokens[index - 1]) {
                let mut base_start = index - 1;
                while base_start >= 2 && tokens[base_start - 1] == "." && is_identifier(&tokens[base_start - 2]) {
                    base_start -= 2;
                }
                let receiver = tokens[index - 1].as_str();
                let is_resource = if index >= base_start + 3 && tokens.get(base_start + 1).map(String::as_str) == Some(".") {
                    let structure = resource_instances.get(&tokens[base_start]);
                    structure.is_some_and(|structure| resource_fields.iter()
                        .any(|field| &field.structure == structure && field.field == receiver))
                } else {
                    resource_shapes.contains_key(receiver)
                };
                if is_resource {
                    return Err(format!("HLSL texture method {method} is not supported by the legacy renderer"));
                }
            }
            rewritten.push(tokens[index].clone());
            index += 1;
            continue;
        }
        if index == 0 || !is_identifier(&tokens[index - 1]) {
            return Err("HLSL resource methods require a named texture object".into());
        }
        let mut base_start = index - 1;
        while base_start >= 2 && tokens[base_start - 1] == "." && is_identifier(&tokens[base_start - 2]) {
            base_start -= 2;
        }
        let base_name = tokens[index - 1].clone();
        let resource_shape = if index >= base_start + 3 && tokens.get(base_start + 1).map(String::as_str) == Some(".") {
            let structure = resource_instances.get(&tokens[base_start]);
            let field_name = &tokens[index - 1];
            structure.and_then(|structure| {
                resource_fields.iter().find(|field| &field.structure == structure && &field.field == field_name)
                    .and_then(|_| resource_shapes.get(&format!("{structure}.{field_name}")))
                    .copied().flatten()
            })
        } else {
            resource_shapes.get(&base_name).copied().flatten()
        }.ok_or_else(|| format!("HLSL resource method receiver {base_name} has no unambiguous Texture1D/Texture2D/Texture3D declaration"))?;
        let dimension = resource_shape.dimension;
        let (args, end) = split_hlsl_call(tokens, index + 2)?;
        if method == "GetDimensions" {
            let expected = usize::from(dimension);
            let has_mip = args.len() == expected + 2;
            if (args.len() != expected && !has_mip) || args.iter().any(Vec::is_empty) {
                return Err("HLSL GetDimensions accepts base-level outputs or MipLevel plus NumberOfLevels".into());
            }
            if tokens.get(end).map(String::as_str) != Some(";") {
                return Err("HLSL GetDimensions must be a standalone statement in this profile".into());
            }
            rewritten.truncate(rewritten.len().saturating_sub(index - base_start));
            let receiver = tokens[base_start..index].join(" ");
            let mip_variable = if has_mip {
                let mut suffix = 0usize;
                loop {
                    let candidate = format!("milkRustTextureMip{suffix}");
                    if !tokens.iter().any(|token| token == &candidate) { break candidate; }
                    suffix = suffix.saturating_add(1);
                }
            } else { String::new() };
            if has_mip {
                rewritten.extend(["{".into(), "int".into(), mip_variable.clone(), "=".into()]);
                rewritten.extend(args[0].clone());
                rewritten.push(";".into());
            }
            for axis in 0..expected {
                let target = &args[axis + if has_mip { 1 } else { 0 }];
                rewritten.extend(target.clone());
                rewritten.push("=".into());
                rewritten.push(if dimension <= 2 { "tex2Dsize" } else { "tex3Dsize" }.into());
                rewritten.push("(".into());
                rewritten.extend_from_slice(&tokens[base_start..index]);
                if has_mip {
                    rewritten.push(",".into());
                    rewritten.push(mip_variable.clone());
                }
                rewritten.push(")".into());
                rewritten.push(".".into());
                rewritten.push(["x", "y", "z"][axis].into());
                rewritten.push(";".into());
            }
            if has_mip {
                let target = args.last().ok_or_else(|| "Malformed HLSL mip-level GetDimensions".to_string())?;
                let size = if dimension == 1 {
                    format!("tex2Dsize({receiver}).x")
                } else if dimension == 2 {
                    format!("max(tex2Dsize({receiver}).x, tex2Dsize({receiver}).y)")
                } else {
                    format!("max(tex3Dsize({receiver}).x, max(tex3Dsize({receiver}).y, tex3Dsize({receiver}).z))")
                };
                let levels = format!("{} = 1 + int(floor(log2(float({size}))));", target.join(" "));
                rewritten.extend(milkrust_hlsl::tokenize(&levels)?);
                rewritten.push("}".into());
            }
            index = end + 1;
            continue;
        }
        let (intrinsic, coordinate): (&str, Vec<String>) = match (method, args.len(), dimension) {
            ("Sample", 2, 1 | 2) => ("tex2D", args[1].clone()),
            ("Sample", 2, 3) => ("tex3D", args[1].clone()),
            ("Sample", 3, 1 | 2) => ("tex2Doffset", args[1].clone()),
            ("Sample", 3, 3) => ("tex3Doffset", args[1].clone()),
            ("SampleLevel", 3 | 4, 1 | 2) => ("tex2Dlod", args[1].clone()),
            ("SampleLevel", 3 | 4, 3) => ("tex3Dlod", args[1].clone()),
            ("SampleBias", 3 | 4, 1 | 2) => ("tex2Dbias", args[1].clone()),
            ("SampleBias", 3 | 4, 3) => ("tex3Dbias", args[1].clone()),
            ("SampleGrad", 4 | 5, 1 | 2) => ("tex2Dgrad", args[1].clone()),
            ("SampleGrad", 4 | 5, 3) => ("tex3Dgrad", args[1].clone()),
            ("Load", 1 | 2, 1 | 2) => ("tex2Dfetch", args[0].clone()),
            ("Load", 1 | 2, 3) => ("tex3Dfetch", args[0].clone()),
            _ => return Err(format!("Invalid argument count for HLSL texture method {method}")),
        };
        if args.iter().any(Vec::is_empty) { return Err(format!("Empty argument to HLSL texture method {method}")); }
        let sampler_state_name = if args[0].len() == 1 {
            sampler_state_names.contains(&args[0][0]).then(|| args[0][0].clone())
        } else if args[0].len() == 3 && is_identifier(&args[0][0]) && args[0][1] == "." && is_identifier(&args[0][2]) {
            sampler_state_instances.get(&args[0][0]).and_then(Option::as_ref)
                .and_then(|structure| sampler_state_fields.get(structure))
                .is_some_and(|fields| fields.contains(&args[0][2]))
                .then(|| format!("{}.{}", args[0][0], args[0][2]))
        } else {
            None
        };
        if method != "Load" && sampler_state_name.is_none() {
            return Err(format!("HLSL texture method {method} requires a declared SamplerState name or direct struct field"));
        }
        let sampler_profile = sampler_state_name.as_ref().and_then(|name| sampler_state_profiles.get(name)).copied();
        rewritten.truncate(rewritten.len().saturating_sub(index - base_start));
        rewritten.push(intrinsic.into());
        rewritten.push("(".into());
        if let Some(profile) = sampler_profile {
            let binding_name = if index >= base_start + 3 && tokens.get(base_start + 1).map(String::as_str) == Some(".") {
                format!("{}_{}", tokens[base_start], base_name)
            } else {
                base_name.clone()
            };
            let resource_name = crate::resources::base_name(&binding_name);
            let alias = profile.binding_name(resource_name, dimension)?;
            if tokens.iter().any(|token| token == &alias) {
                return Err(format!("Generated HLSL sampler binding {alias} collides with a user identifier"));
            }
            rewritten.push(alias);
        } else {
            rewritten.extend_from_slice(&tokens[base_start..index]);
        }
        rewritten.push(",".into());
        if method == "SampleLevel" || method == "SampleBias" {
            rewritten.push("float4".into());
            rewritten.push("(".into());
            if dimension == 1 {
                rewritten.extend(["float2".into(), "(".into()]);
                rewritten.extend(coordinate);
                rewritten.extend([",".into(), "0.5".into(), ")".into()]);
            } else {
                rewritten.extend(coordinate);
            }
            if dimension == 2 {
                rewritten.extend([",".into(), "0.0".into()]);
            }
            if dimension == 1 {
                // `float4(float2(u, 0.5), 0.0, lod)` keeps the LOD in `.w`.
                rewritten.extend([",".into(), "0.0".into()]);
            }
            rewritten.push(",".into());
            rewritten.extend(args[2].clone());
            rewritten.push(")".into());
        } else {
            if dimension == 1 {
                rewritten.extend(if method == "Load" { vec!["int2".into()] } else { vec!["float2".into()] });
                rewritten.push("(".into());
                rewritten.extend(coordinate);
                rewritten.extend([",".into(), if method == "Load" { "0" } else { "0.5" }.into(), ")".into()]);
            } else {
                rewritten.extend(coordinate);
            }
        }
        if method == "SampleGrad" {
            rewritten.push(",".into());
            if dimension == 1 {
                rewritten.extend(["float2".into(), "(".into()]);
                rewritten.extend(args[2].clone());
                rewritten.extend([",".into(), "0.0".into(), ")".into()]);
            } else {
                rewritten.extend(args[2].clone());
            }
            rewritten.push(",".into());
            if dimension == 1 {
                rewritten.extend(["float2".into(), "(".into()]);
                rewritten.extend(args[3].clone());
                rewritten.extend([",".into(), "0.0".into(), ")".into()]);
            } else {
                rewritten.extend(args[3].clone());
            }
        }
        let offset_index = match (method, args.len()) {
            ("Sample", 3) => Some(2),
            ("SampleLevel", 4) | ("SampleBias", 4) => Some(3),
            ("SampleGrad", 5) => Some(4),
            ("Load", 2) => Some(1),
            _ => None,
        };
        if let Some(offset) = offset_index.and_then(|offset| args.get(offset)) {
            rewritten.push(",".into());
            if dimension == 1 {
                rewritten.extend(["int2".into(), "(".into()]);
                rewritten.extend(offset.clone());
                rewritten.extend([",".into(), "0".into(), ")".into()]);
            } else {
                rewritten.extend(offset.clone());
            }
        }
        rewritten.push(")".into());
        if resource_shape.width < 4 {
            rewritten.push(".".into());
            rewritten.push(match resource_shape.width { 1 => "x", 2 => "xy", _ => "xyz" }.into());
        }
        index = end;
    }
    // Static legacy sampler declarations bind a resource and sampler state in
    // one effect-file block. Rewrite only direct tex* sampler arguments so an
    // opaque state cannot escape into unsupported arithmetic or assignments.
    for index in 0..rewritten.len() {
        let Some(alias) = legacy_sampler_bindings.get(&rewritten[index]).cloned() else { continue; };
        let valid_intrinsic = index >= 2
            && rewritten[index - 1] == "("
            && LEGACY_SAMPLER_INTRINSICS.contains(&rewritten[index - 2].as_str());
        if !valid_intrinsic {
            return Err(format!("Legacy sampler_state variable {} may only be used as the first argument to a tex* intrinsic", rewritten[index]));
        }
        rewritten[index] = alias;
    }
    *tokens = rewritten;
    Ok(())
}

pub(crate) fn sampler_names(source: &str) -> Result<Vec<String>, String> {
    let source = milkrust_hlsl::preprocess(source)?;
    let mut tokens = milkrust_hlsl::tokenize(&source)?;
    lower_modern_resource_syntax(&mut tokens)?;
    let mut names = Vec::new();
    let mut sampler_parameters = Vec::new();
    let mut parens = 0i32;
    let mut braces = 0i32;
    let mut functions = Vec::new();
    let structures: Vec<&String> = tokens.windows(2).filter(|pair| pair[0] == "struct")
        .map(|pair| &pair[1]).collect();
    let is_return_type = |name: &String| {
        structures.contains(&name) || ["void", "float", "half", "int", "uint", "bool", "double", "unsigned", "dword",
            "min16float", "min10float", "min12int", "min16int", "min16uint"]
            .iter().any(|base| name.strip_prefix(*base).is_some_and(|suffix|
                suffix.is_empty() || (suffix.len() <= 3 && suffix.bytes().all(|b| matches!(b, b'1'..=b'4' | b'x')))))
    };
    for (index, token) in tokens.iter().enumerate() {
        if token == "(" && parens == 0 && braces == 0 && index >= 2
            && is_return_type(&tokens[index - 2]) {
            functions.push(tokens[index - 1].clone());
        }
        if token == "(" { parens += 1; }
        if token == ")" { parens -= 1; }
        if token == "{" { braces += 1; }
        if token == "}" { braces -= 1; }
        if parens > 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D") {
            if let Some(name) = tokens.get(index + 1) { sampler_parameters.push(name); }
        } else if parens == 0 && braces == 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D") {
            let mut declarator = index + 1;
            loop {
                let Some(name) = tokens.get(declarator).filter(|name| is_identifier(name)) else { break; };
                if !crate::is_milkrust_main_sampler(name) && !names.contains(name) { names.push(name.clone()); }
                if tokens.get(declarator + 1).map(String::as_str) != Some(",") { break; }
                declarator += 2;
            }
        }
    }
    // Only referenced resources are reflected, never arbitrary identifiers in
    // unknown function calls. Sampler parameters belong to their function.
    for call in tokens.windows(4) {
        if LEGACY_SAMPLER_INTRINSICS.contains(&call[0].as_str())
            && call[1] == "(" && (call[3] == "," || (matches!(call[0].as_str(), "tex2Dsize" | "tex3Dsize") && call[3] == ")")) && !crate::is_milkrust_main_sampler(&call[2])
            && !sampler_parameters.contains(&&call[2]) && !names.contains(&call[2]) {
            names.push(call[2].clone());
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if functions.contains(token) && tokens.get(index + 1).map(String::as_str) == Some("(") {
            let mut depth = 1;
            for argument in tokens.iter().skip(index + 2) {
                if argument == "(" { depth += 1; }
                if argument == ")" { depth -= 1; if depth == 0 { break; } }
                if argument.starts_with("sampler_") && !sampler_parameters.contains(&argument)
                    && !crate::is_milkrust_main_sampler(argument) && !names.contains(argument) {
                    names.push(argument.clone());
                }
            }
        }
    }
    for level in 1..=3 {
        let name = format!("sampler_blur{level}");
        if tokens.iter().any(|t| t == &format!("GetBlur{level}")) && !names.contains(&name) { names.push(name); }
    }
    for token in &tokens {
        if let Some(base) = token.strip_prefix("texsize_") {
            let name = format!("sampler_{base}");
            if base != "main" && !crate::resources::VECTOR_UNIFORMS.contains(&token.as_str()) && !names.contains(&name) { names.push(name); }
        }
    }
    for name in resource_field_bindings(&tokens)? {
        if !names.contains(&name) { names.push(name); }
    }
    if names.len() > 15 { return Err("Shader exceeds 15 additional samplers".into()); }
    Ok(names)
}

#[derive(Clone)]
struct ResourceStructField {
    structure: String,
    field: String,
    sampler_type: String,
    declaration: std::ops::Range<usize>,
}

fn resource_struct_fields(tokens: &[String]) -> Result<(Vec<ResourceStructField>, BTreeMap<String, String>, BTreeMap<String, String>), String> {
    let mut fields = Vec::new();
    let mut structures = BTreeMap::new();
    let mut structure_ranges = Vec::new();
    let mut i = 0;
    let mut depth = 0i32;
    while i + 2 < tokens.len() {
        if tokens[i] == "{" { depth += 1; i += 1; continue; }
        if tokens[i] == "}" { depth -= 1; i += 1; continue; }
        if depth != 0 || tokens[i] != "struct" { i += 1; continue; }
        let name = tokens[i + 1].clone();
        let Some(open) = tokens[i + 2..].iter().position(|token| token == "{").map(|offset| i + 2 + offset) else {
            return Err("Malformed HLSL struct declaration".into());
        };
        let mut level = 1i32;
        let mut close = open + 1;
        while close < tokens.len() && level > 0 {
            if tokens[close] == "{" { level += 1; }
            if tokens[close] == "}" { level -= 1; }
            close += 1;
        }
        if level != 0 { return Err("Unterminated HLSL struct declaration".into()); }
        structures.insert(name.clone(), String::new());
        structure_ranges.push(i..close);
        let mut field_index = open + 1;
        while field_index + 1 < close - 1 {
            if !matches!(tokens[field_index].as_str(), "sampler" | "sampler2D" | "sampler3D") {
                field_index += 1;
                continue;
            }
            let sampler_type = tokens[field_index].clone();
            let field_name = tokens[field_index + 1].clone();
            let Some(end_offset) = tokens[field_index + 2..close - 1].iter().position(|token| token == ";") else {
                return Err("Malformed sampler field in HLSL struct".into());
            };
            let end = field_index + 2 + end_offset;
            if tokens[field_index + 2..end].iter().any(|token| matches!(token.as_str(), "[" | "=")) {
                return Err("Sampler arrays and initialized sampler fields in structs are unsupported".into());
            }
            let mut field_names = vec![field_name];
            let mut cursor = field_index + 2;
            while cursor < end {
                if tokens[cursor] == "," {
                    let Some(next_name) = tokens.get(cursor + 1).filter(|token| token.as_str() != ";") else {
                        return Err("Malformed sampler field declaration in HLSL struct".into());
                    };
                    field_names.push(next_name.clone());
                    cursor += 2;
                } else { cursor += 1; }
            }
            let declaration_start = if field_index > open + 1 && tokens[field_index - 1] == "uniform" { field_index - 1 } else { field_index };
            for field_name in field_names {
                fields.push(ResourceStructField {
                    structure: name.clone(), field: field_name, sampler_type: sampler_type.clone(),
                    declaration: declaration_start..end + 1,
                });
            }
            field_index = end + 1;
        }
        i = close;
    }
    if fields.is_empty() { return Ok((fields, BTreeMap::new(), BTreeMap::new())); }

    let mut instances = BTreeMap::new();
    depth = 0;
    let mut parens = 0i32;
    let mut i = 0;
    while i + 2 < tokens.len() {
        if let Some(range) = structure_ranges.iter().find(|range| range.start == i) {
            i = range.end;
            continue;
        }
        if depth == 0 && parens == 0 && structures.contains_key(&tokens[i])
            && tokens.get(i + 1).is_some_and(|name| name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')) {
            let structure = tokens[i].clone();
            let mut declaration = i + 1;
            loop {
                let variable = tokens[declaration].clone();
                let next = tokens.get(declaration + 1).map(String::as_str);
                if next == Some("[") { return Err("Arrays of resource-bearing structs are unsupported".into()); }
                if next == Some("=") { return Err("Initialized resource-bearing struct instances are unsupported".into()); }
                instances.insert(variable, structure.clone());
                match next {
                    Some(",") => declaration += 2,
                    Some(";") => break,
                    _ => { instances.remove(&tokens[declaration]); break; }
                }
            }
        }
        if tokens[i] == "{" { depth += 1; }
        if tokens[i] == "}" { depth -= 1; }
        if tokens[i] == "(" { parens += 1; }
        if tokens[i] == ")" { parens -= 1; }
        i += 1;
    }

    let mut bindings = BTreeMap::new();
    for window in tokens.windows(3) {
        if window[1] != "." { continue; }
        let field = fields.iter().find(|field| field.field == window[2]);
        let Some(structure) = instances.get(&window[0]) else {
            if field.is_some() { return Err("Resource struct fields must be accessed through a global struct instance".into()); }
            continue;
        };
        if let Some(field) = fields.iter().find(|field| &field.structure == structure && field.field == window[2]) {
            let name = if field.field.starts_with("sampler_") { field.field.clone() }
                else { format!("sampler_{}_{}", window[0], field.field) };
            if field.sampler_type == "sampler2D" && crate::resources::texture_kind(&name).is_volume()
                || field.sampler_type == "sampler3D" && !crate::resources::texture_kind(&name).is_volume() {
                return Err(format!("Sampler field {}.{} disagrees with its bound texture dimension", window[0], field.field));
            }
            bindings.insert(format!("{}.{}", window[0], field.field), name.clone());
        }
    }
    Ok((fields, bindings, instances))
}

fn resource_field_bindings(tokens: &[String]) -> Result<Vec<String>, String> {
    let (_, bindings, _) = resource_struct_fields(tokens)?;
    let mut names = Vec::new();
    for window in tokens.windows(3) {
        if window[1] == "." {
            if let Some(name) = bindings.get(&format!("{}.{}", window[0], window[2])) {
                if !names.contains(name) { names.push(name.clone()); }
            }
        }
    }
    Ok(names)
}

fn lower_resource_struct_fields(tokens: &mut Vec<String>) -> Result<(), String> {
    let (fields, bindings, instances) = resource_struct_fields(tokens)?;
    if fields.is_empty() { return Ok(()); }
    let resource_struct_names: Vec<String> = fields.iter().map(|field| field.structure.clone()).collect();
    let mut removed = vec![false; tokens.len()];
    for field in fields {
        for index in field.declaration { removed[index] = true; }
    }
    let mut empty_structs = Vec::new();
    let mut struct_ranges = Vec::new();
    let mut index = 0;
    while index + 2 < tokens.len() {
        if tokens[index] != "struct" { index += 1; continue; }
        let name = tokens[index + 1].clone();
        let Some(open) = tokens[index + 2..].iter().position(|token| token == "{").map(|offset| index + 2 + offset) else { break; };
        let mut level = 1i32;
        let mut close = open + 1;
        while close < tokens.len() && level > 0 {
            if tokens[close] == "{" { level += 1; }
            if tokens[close] == "}" { level -= 1; }
            close += 1;
        }
        if level != 0 { return Err("Unterminated HLSL struct declaration".into()); }
        if resource_struct_names.contains(&name) && (open + 1..close - 1).all(|field| removed[field]) {
            empty_structs.push(name);
            let end = if tokens.get(close).map(String::as_str) == Some(";") { close + 1 } else { close };
            for token in index..end { removed[token] = true; }
            struct_ranges.push(index..end);
            index = end;
        } else { index = close; }
    }
    for (object, structure) in &instances {
        if !empty_structs.contains(structure) { continue; }
        let mut index = 0;
        let mut depth = 0i32;
        let mut parens = 0i32;
        while index < tokens.len() {
            if let Some(range) = struct_ranges.iter().find(|range| range.start == index) {
                index = range.end;
                continue;
            }
            if removed[index] { index += 1; continue; }
            if depth == 0 && parens == 0 && tokens[index] == *structure && tokens.get(index + 1) == Some(object)
                && matches!(tokens.get(index + 2).map(String::as_str), Some(";") | Some(",")) {
                let end = tokens[index + 2..].iter().position(|token| token == ";")
                    .map(|offset| index + 2 + offset).ok_or_else(|| "Unterminated resource struct declaration".to_string())?;
                for token in index..=end { removed[token] = true; }
                index = end + 1;
                continue;
            }
            if tokens[index] == *object {
                if tokens.get(index + 1).map(String::as_str) != Some(".")
                    || tokens.get(index + 2).map_or(true, |field| !bindings.contains_key(&format!("{object}.{field}"))) {
                    return Err("Resource-only struct instances must be accessed through declared sampler fields".into());
                }
            }
            if tokens[index] == "{" { depth += 1; }
            if tokens[index] == "}" { depth -= 1; }
            if tokens[index] == "(" { parens += 1; }
            if tokens[index] == ")" { parens -= 1; }
            index += 1;
        }
    }
    let mut rewritten = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        if index + 2 < tokens.len() && tokens[index + 1] == "." {
            if let Some(name) = bindings.get(&format!("{}.{}", tokens[index], tokens[index + 2])) {
                rewritten.push(name.clone()); index += 3; continue;
            }
        }
        if !removed[index] { rewritten.push(tokens[index].clone()); }
        index += 1;
    }
    *tokens = rewritten;
    Ok(())
}

pub(crate) fn needs_environment(source: &str) -> bool {
    let Ok(tokens) = milkrust_hlsl::tokenize(source) else { return true; };
    tokens.iter().any(|name| crate::resources::MATRIX_UNIFORMS.contains(&name.as_str())
        || crate::resources::VECTOR_UNIFORMS.contains(&name.as_str())
        || crate::resources::SCALAR_UNIFORMS.contains(&name.as_str())
        || name.starts_with("mip_") || name.starts_with("texsize_")
        || name.strip_prefix('q').and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| (1..=64).contains(&n))
        || ["rad", "ang", "aspect", "uv_orig", "hue_shader", "GetMain", "GetPixel", "GetBlur1", "GetBlur2", "GetBlur3", "lum",
            "M_PI", "M_PI_2", "M_INV_PI_2", "M_INV_PI", "tex2d", "tex3d"].contains(&name.as_str()))
}

pub(crate) fn compile(source: &str, legacy_feedback: bool) -> Result<String, String> {
    if source.len() > 128 * 1024 { return Err("Shader source exceeds 128 KiB".into()); }
    let source = milkrust_hlsl::preprocess(source)?;
    let samplers = sampler_names(&source)?;
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    aliases.insert("tex".into(), "tex2D".into());
    aliases.insert("tex2d".into(), "tex2D".into());
    aliases.insert("tex3d".into(), "tex3D".into());
    aliases.insert("aspect".into(), "hlslAspect".into());
    aliases.insert("uv".into(), "milkrust_uv_value".into());
    aliases.insert("uv_orig".into(), "milkrust_uv_orig_value".into());
    aliases.insert("texsize_main".into(), "texsize".into());
    for q in 0..64 {
        aliases.insert(format!("q{}", q+1), format!("_q{}.{}", char::from(b'a'+(q/4) as u8), ["x", "y", "z", "w"][q%4]));
    }
    for (name, value) in [("M_PI", "3.141592653589793"), ("M_PI_2", "6.283185307179586"),
        ("M_INV_PI_2", "0.159154943091895"), ("M_INV_PI", "0.318309886183791"), ("mip_x", "_c12.x"), ("mip_y", "_c12.y"),
        ("mip_xy", "_c12.xy"), ("mip_avg", "_c12.z")] { aliases.insert(name.into(), value.into()); }
    for name in ["sampler_main"] {
        aliases.insert(name.into(), "previousFrame".into());
    }
    for (index, name) in samplers.iter().enumerate() { aliases.insert(name.clone(), format!("shaderTexture{index}")); }
    // GLSL spellings accepted by the original expression path are also valid
    // in snippets. Half/minimum precision is explicitly widened to f32.
    for (from, to) in [("vec2", "float2"), ("vec3", "float3"), ("vec4", "float4"),
        ("float1", "float"), ("half1", "float"), ("min16float", "float"),
        ("min16float2", "float2"), ("min16float3", "float3"), ("min16float4", "float4"),
        ("unsigned", "uint"), ("dword", "uint"), ("dword2", "uint2"), ("dword3", "uint3"), ("dword4", "uint4")] {
        aliases.insert(from.into(), to.into());
    }
    for (prefix, target) in [("min10float", "float"), ("min12int", "int"),
        ("min16int", "int"), ("min16uint", "uint")] {
        aliases.insert(prefix.into(), target.into());
        for width in 1..=4 {
            let vector_type = if width == 1 { target.to_string() } else { format!("{target}{width}") };
            aliases.insert(format!("{prefix}{width}"), vector_type);
            for columns in 1..=4 {
                aliases.insert(format!("{prefix}{width}x{columns}"), format!("{target}{width}x{columns}"));
            }
        }
    }
    for width in 1..=4 {
        let vector_type = if width == 1 { "float".to_string() } else { format!("float{width}") };
        aliases.insert(format!("min16float{width}"), vector_type.clone());
        aliases.insert(format!("min10float{width}"), if width == 1 { "float".into() } else { format!("float{width}") });
        for columns in 1..=4 {
            aliases.insert(format!("min16float{width}x{columns}"), format!("float{width}x{columns}"));
            aliases.insert(format!("min10float{width}x{columns}"), format!("float{width}x{columns}"));
        }
    }
    // Rename any user entry before the emitter adds its GLSL main.
    aliases.insert("main".into(), "milkrust_user_entry".into());
    let mut tokens = milkrust_hlsl::tokenize(&source)?;
    lower_modern_resource_syntax(&mut tokens)?;
    lower_resource_struct_fields(&mut tokens)?;
    let mut index = 0;
    while index + 1 < tokens.len() {
        if tokens[index] == "unsigned" && tokens[index + 1] == "int" {
            tokens[index] = "uint".into();
            tokens.remove(index + 1);
        }
        index += 1;
    }
    for pair in tokens.windows(2) {
        if matches!(pair[0].as_str(), "float" | "double" | "half") && matches!(pair[1].as_str(), "M_PI" | "M_PI_2" | "M_INV_PI_2" | "M_INV_PI") {
            aliases.remove(&pair[1]);
        }
    }
    if tokens.iter().any(|t| t.starts_with("milkrust_") || t.starts_with("gl_") || t.starts_with("rast_") || t.starts_with("frag_")) {
        return Err("Shader uses a reserved compiler identifier".into());
    }
    for token in &mut tokens { if let Some(alias) = aliases.get(token) { *token = alias.clone(); } }
    // Sampler declarations saved in a preamble are represented by the exact
    // host binding below. Remove only complete, top-level bound declarations.
    let mut clean = Vec::new();
    let mut index = 0;
    let mut depth = 0i32;
    let mut parens = 0i32;
    while index < tokens.len() {
        let token = &tokens[index];
        if depth == 0 && parens == 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D")
            && tokens.get(index + 1).is_some_and(|name| name == "previousFrame" || name.starts_with("shaderTexture")) {
            let name = &tokens[index + 1];
            let expected_volume = name.strip_prefix("shaderTexture").and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| samplers.get(n)).is_some_and(|n| crate::resources::texture_kind(n).is_volume());
            if (token == "sampler3D") != expected_volume && token != "sampler" { return Err("Sampler dimension disagrees with its bound texture".into()); }
            index += 2;
            while index < tokens.len() && tokens[index] != ";" {
                if tokens[index] == "{" || tokens[index] == "=" || tokens[index] == "," { return Err("Custom sampler state declarations are unsupported".into()); }
                index += 1;
            }
            if index == tokens.len() { return Err("Unterminated sampler declaration".into()); }
            index += 1; continue;
        }
        if token == "{" { depth += 1; } else if token == "}" { depth -= 1; }
        if token == "(" { parens += 1; } else if token == ")" { parens -= 1; }
        clean.push(token.clone()); index += 1;
    }
    tokens = clean;
    let explicit_entry = tokens.iter().any(|t| t == "milkrust_user_entry");
    let mut preamble = String::from("float3 color; sampler2D previousFrame; float feedback; float outputAlpha; float time; float sampleRate; float fftBins[64]; float waveformBins[64]; float2 resolution; float2 pixelSize; float4 hlslAspect; float4 texsize;\n");
    preamble.push_str("static float2 milkrust_uv_value; static float2 milkrust_uv_orig_value; static float x; static float y; static float rad; static float ang; static float3 ret; static float3 hue_shader;\n");
    for name in ["bass", "bass_att", "mid", "mid_att", "treb", "treb_att"] { preamble.push_str(&format!("float {name};\n")); }
    for q in 1..=64 { preamble.push_str(&format!("float q{q};\n")); }
    for (index, name) in samplers.iter().enumerate() {
        let kind = if crate::resources::texture_kind(name).is_volume() { "sampler3D" } else { "sampler2D" };
        preamble.push_str(&format!("{kind} shaderTexture{index};\n"));
    }
    for name in crate::resources::SCALAR_UNIFORMS { preamble.push_str(&format!("float {name};\n")); }
    for name in crate::resources::VECTOR_UNIFORMS { preamble.push_str(&format!("float4 {name};\n")); }
    for name in crate::resources::MATRIX_UNIFORMS { preamble.push_str(&format!("float4x3 {name};\n")); }
    let sizes: std::collections::BTreeSet<_> = samplers.iter().map(|name| format!("texsize_{}", crate::resources::base_name(name))).collect();
    for name in sizes {
        if !crate::resources::VECTOR_UNIFORMS.contains(&name.as_str()) { preamble.push_str(&format!("float4 {name};\n")); }
    }
    for level in 1..=3 {
        if let Some(index) = samplers.iter().position(|name| name == &format!("sampler_blur{level}")) {
            let (scale, bias) = match level { 1 => ("_c5.x", "_c5.y"), 2 => ("_c5.z", "_c5.w"), _ => ("_c6.x", "_c6.y") };
            preamble.push_str(&format!("float3 GetBlur{level}(float2 p) {{ return tex2D(shaderTexture{index}, p).rgb * {scale} + {bias}; }}\n"));
        }
    }
    preamble.push_str("float3 GetMain(float2 p) { return tex2D(previousFrame, p).rgb; }\nfloat3 GetPixel(float2 p) { return GetMain(p); }\nfloat lum(float3 c) { return dot(c, float3(0.32, 0.49, 0.29)); }\n");
    preamble.push_str("float get_fft(float p) { return fftBins[(int)(clamp(p, 0.0, 1.0)*63.0)]; }\nfloat get_fft_hz(float hz) { return get_fft(hz / max(sampleRate * 0.5, 1.0)); }\nfloat get_waveform(float p) { return waveformBins[(int)(clamp(p,0.0,1.0)*63.0)]; }\n");
    // Delimits host-owned uniforms from user globals in the typed AST. The
    // compiler emits user globals as per-fragment storage and initializes them
    // after the adapter has populated the shared environment.
    preamble.push_str("static const int milkrust_host_end = 0;\n");
    let entry;
    if explicit_entry {
        // The adapter maps legacy UV pairs, polar coordinates and hue input,
        // plus system fragment coordinates. Other semantics fail below.
        entry = "milkrust_user_entry";
        preamble.push_str(&tokens.join(" "));
    } else {
        entry = "milkrust_entry";
        let body = if let Some(marker) = tokens.iter().position(|t| t == "shader_body") {
            preamble.push_str(&tokens[..marker].join(" "));
            if tokens.get(marker + 1).map(String::as_str) != Some("{") || tokens.last().map(String::as_str) != Some("}") {
                return Err("Malformed shader_body".into());
            }
            tokens[marker + 2..tokens.len() - 1].join(" ")
        } else if tokens.first().map(String::as_str) == Some("{") && tokens.last().map(String::as_str) == Some("}") {
            tokens[1..tokens.len() - 1].join(" ")
        } else { tokens.join(" ") };
        preamble.push_str(&format!(r#"
float3 milkrust_body() {{
    {body}
    return ret;
}}
float4 milkrust_entry(float2 milkrust_uv : TEXCOORD0) : COLOR0 {{
    return float4(milkrust_body(), 1.0);
}}
"#));
    }
    let glsl = milkrust_hlsl::compile(&preamble, entry)?;
    adapt_output(&glsl, legacy_feedback)
}

fn adapt_output(glsl: &str, feedback: bool) -> Result<String, String> {
    // Validate the generated interface before changing it. No unknown varying
    // or render target is silently initialized with zero.
    let mut output = String::new();
    let mut has_uv = false;
    let mut has_output = false;
    let mut has_main = false;
    let mut initializers = Vec::new();
    for line in glsl.lines() {
        let trimmed = line.trim();
        // Keep the host binding spelling, then copy into mutable per-fragment
        // storage. Helpers and user assignments see the same local registers.
        if let Some(declaration) = trimmed.strip_prefix("uniform ").and_then(|v| v.strip_suffix(';')) {
            let parts: Vec<_> = declaration.split_whitespace().collect();
            if parts.len() == 2 {
                let (kind, name) = (parts[0], parts[1]);
                let mutable = crate::resources::SCALAR_UNIFORMS.contains(&name)
                    || crate::resources::VECTOR_UNIFORMS.contains(&name)
                    || crate::resources::MATRIX_UNIFORMS.contains(&name)
                    || ["color", "time", "texsize", "hlslAspect", "bass", "mid", "treb", "bass_att", "mid_att", "treb_att"].contains(&name)
                    || name.strip_prefix('q').and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| (1..=64).contains(&n));
                if mutable {
                    let suffix = name.trim_start_matches('_');
                    output.push_str(&format!("{line}\n{kind} milkrust_value_{suffix};\nvoid milkrust_init_{suffix}() {{ milkrust_value_{suffix} = {name}; }}\n#define {name} milkrust_value_{suffix}\n"));
                    initializers.push(format!("milkrust_init_{suffix}();"));
                    continue;
                }
            }
        }
        if trimmed.starts_with("in ") {
            if trimmed == "in vec2 frag_TEXCOORD0;" {
                output.push_str("in vec2 uv;\nvec2 milkrust_input_uv() { return uv; }\n#define frag_TEXCOORD0 milkrust_input_uv()\n");
                has_uv = true; continue;
            }
            if trimmed == "in vec4 frag_TEXCOORD0;" {
                output.push_str("in vec2 uv;\n#define frag_TEXCOORD0 vec4(uv, originalUv)\n");
                has_uv = true; continue;
            }
            if trimmed == "in vec2 frag_TEXCOORD1;" { output.push_str("#define frag_TEXCOORD1 vec2(rad, ang)\n"); continue; }
            if trimmed == "in vec4 frag_COLOR0;" { output.push_str("#define frag_COLOR0 vec4(hue_shader, 1.0)\n"); continue; }
            if trimmed == "in vec3 frag_COLOR0;" { output.push_str("#define frag_COLOR0 hue_shader\n"); continue; }
            return Err(format!("Unsupported pixel shader input: {trimmed}"));
        }
        if trimmed.starts_with("out ") {
            if trimmed != "out vec4 rast_FragData[1];" { return Err(format!("Unsupported pixel shader output: {trimmed}")); }
            has_output = true;
        }
        if trimmed == "void main() {" {
            output.push_str("void milkrust_generated_main() {\n");
            has_main = true; continue;
        }
        output.push_str(line); output.push('\n');
    }
    if !has_output || !has_main { return Err("Shader entry must write COLOR0".into()); }
    if !has_uv { output.push_str("in vec2 uv;\n"); }
    output.insert_str(output.find('\n').map_or(0, |i| i+1), "in highp vec2 originalUv;\n");
    output.push_str(milk2::GLSL_TRANSITION_MASK);
    // The driver may optimize an unused source uniform out. In GLSL it still
    // exists in the declaration stream emitted from our preamble.
    output.push_str("\nvoid main() {\n");
    output.push_str(&initializers.join("\n"));
    output.push_str("\nmilkrust_uv_value=uv; milkrust_uv_orig_value=originalUv; x=uv.x; y=uv.y;\nvec2 milkrust_centered=(originalUv*2.0-1.0)*hlslAspect.xy;\nrad=length(milkrust_centered)/length(hlslAspect.xy); ang=atan(milkrust_centered.y,milkrust_centered.x); if(ang<0.0)ang+=6.283185307179586;\nhue_shader=mix(mix(hue_corner4.xyz,hue_corner3.xyz,originalUv.x),mix(hue_corner2.xyz,hue_corner1.xyz,originalUv.x),originalUv.y); ret=color;\nmilkrust_generated_main();\n");
    if feedback { output.push_str("rast_FragData[0].rgb = mix(rast_FragData[0].rgb, texture(previousFrame, clamp(uv, vec2(0.0), vec2(1.0))).rgb, feedback);\n"); }
    output.push_str("rast_FragData[0].a *= outputAlpha * milkrust_transition_mask(uv);\n}\n");
    if output.len() > 256 * 1024 { return Err("Generated shader exceeds renderer source limit".into()); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_milkdrop_environment_and_distinct_main_sampler_modes() {
        let source = "shader_body { float3 p=mul(float4(uv,0,1),rot_s1); ret=GetMain(uv)+GetPixel(uv)+GetBlur2(uv); ret+=tex2d(sampler_PC_main,uv).rgb; ret+=float3(rad,ang/M_PI_2,mip_avg)+_c3.xyz+_qa.xyz+p+lum(ret); }";
        assert!(needs_environment(source));
        let output = compile(source, false).unwrap();
        assert!(output.contains("mat3x4 rot_s1"));
        assert!(output.contains("_c5"));
        assert_eq!(sampler_names(source).unwrap(), vec!["sampler_PC_main", "sampler_blur2"]);
    }
    #[test]
    fn helpers_share_mutable_registers_and_fragment_coordinates() {
        let source = "void tint(){q1+=0.25; hue_shader*=2; ret+=float3(uv_orig,rad);}\nshader_body { tint(); ret+=float3(_qa.x,q1,ang); }";
        let result = compile(source, false).unwrap();
        assert!(result.contains("#define _qa milkrust_value_qa"));
        assert!(result.contains("milkrust_init_qa();"));
        assert!(result.contains("milkrust_uv_orig_value=originalUv"));
        assert!(result.contains("hue_shader=mix"));
        assert_eq!(sampler_names("ret=float3(texsize_paper.xy,1);").unwrap(), vec!["sampler_paper"]);
    }
    #[test]
    fn explicit_legacy_entries_receive_uv_pairs_hue_and_polar_coordinates() {
        assert!(compile("float4 main(float4 p:TEXCOORD0,float2 a:TEXCOORD1,float4 hue:COLOR0):COLOR0 { return float4(p.zw,a.x,1)*hue; }",false).is_ok());
    }
    #[test]
    fn initializes_user_globals_after_fragment_environment() {
        let source = "const int COUNT=2; float weights[COUNT]={0.25,0.75}; float3 tint=float3(uv_orig,0.5); float gain; void update(){gain=weights[0]+weights[1];} shader_body { update(); ret=tint*gain; }";
        let result = compile(source, false).unwrap();
        assert!(!result.contains("uniform vec3 tint"));
        assert!(!result.contains("uniform float gain"));
        assert!(result.contains("const int COUNT"));
        assert!(result.contains("weights = float[]"));
        assert!(result.contains("gain = float(0)"));
    }
    #[test]
    fn compiles_typed_helpers_arrays_matrices_and_loops() {
        let result = compile("#define COUNT 3\nfloat3 tint(float3 x) { float3x3 m = float3x3(1,0,0,0,1,0,0,0,1); return mul(m,x); }\nshader_body { float a[3]; a[0]=0.1; a[1]=0.2; a[2]=0.3; for(int i=0;i<COUNT;i++){ ret += tint(float3(a[i],a[i],a[i])); } }", false).unwrap();
        assert!(result.contains("milkrust_generated_main"));
        assert!(result.contains("milkrust_transition_mask"));
        assert!(result.contains("for ("));
    }
    #[test]
    fn discovers_size_only_and_volume_sampling_bindings() {
        assert_eq!(sampler_names("shader_body { ret=float3(tex2Dsize(sampler_art),0); }").unwrap(), vec!["sampler_art"]);
        let source = "shader_body { ret=tex3Dproj(sampler_noisevol_hq,float4(uv,0.5,1)).rgb + tex3Dbias(sampler_noisevol_lq,float4(uv,0.5,0)).rgb; }";
        assert_eq!(sampler_names(source).unwrap(), vec!["sampler_noisevol_hq", "sampler_noisevol_lq"]);
        compile(source, false).expect("volume sampling overloads must compile");
        compile("shader_body { ret=tex2Dlod(sampler_main,float4(uv,0,0),int2(0,0)).rgb; }", false)
            .expect("constant offsets must remain constant after lowering");
    }
    #[test]
    fn preserves_entry_input_and_rejects_unbound_semantics() {
        assert!(compile("float4 main(float2 uv:TEXCOORD0):COLOR0 { return float4(uv,0,1); }", false).is_ok());
        assert!(compile("float4 main(float3 p:NORMAL):COLOR0 { return float4(p,1); }", false).is_err());
    }
    #[test]
    fn discovers_expanded_sampler_bindings() {
        assert_eq!(sampler_names("#define T sampler_cloud\nshader_body { ret=tex2D(T,uv).rgb; }").unwrap(), vec!["sampler_cloud"]);
        assert!(sampler_names("ret=mytex2D(sampler_ignored,uv);").unwrap().is_empty());
        assert_eq!(sampler_names("float3 sample(sampler2D s,float2 p){return tex2D(s,p).rgb;} shader_body {ret=sample(sampler_cloud,uv);}").unwrap(), vec!["sampler_cloud"]);
    }

}

