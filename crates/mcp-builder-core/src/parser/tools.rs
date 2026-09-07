use crate::ast::{
    ExecBinding, OutputSpec, ParamSpec, ParamType, TargetOs, ToolBinding, ToolProfileRef,
    ToolSpec,
};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::bindings::*;
use crate::parser::output::{parse_output_node, validate_output_node};
use crate::parser::profiles::{parse_tool_profile_node, validate_tool_profile_node};
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_TOOL_PROPS: &[&str] = &["name", "description", "profile"];

pub const ALLOWED_TOOL_CHILDREN: &[&str] = &[
    "description",
    "param",
    "output",
    "profile",
    "bind:profile",
    "exec",
    "bind:exec",
    "exec:windows",
    "bind:exec:windows",
    "exec:linux",
    "bind:exec:linux",
    "exec:macos",
    "bind:exec:macos",
    "exec:darwin",
    "bind:exec:darwin",
    "exec:osx",
    "bind:exec:osx",
    "exec:unix",
    "bind:exec:unix",
    "exec:fallback",
    "bind:exec:fallback",
    "exec:default",
    "bind:exec:default",
    "fallback",
    "default",
    "http",
    "bind:http",
    "ipc",
    "bind:ipc",
    "native",
    "bind:native",
    "com",
    "bind:com",
    "ws",
    "bind:ws",
    "websocket",
    "bind:websocket",
    "pipe",
    "bind:pipe",
    "tcp",
    "bind:tcp",
];

pub const ALLOWED_PARAM_PROPS: &[&str] = &[
    "name",
    "type",
    "description",
    "required",
    "default",
    "minimum",
    "maximum",
    "min",
    "max",
    "pattern",
    "enum",
];

pub const ALLOWED_PARAM_CHILDREN: &[&str] = &[
    "type",
    "description",
    "default",
    "minimum",
    "maximum",
    "min",
    "max",
    "pattern",
    "enum",
];

pub const ALLOWED_PARAM_TYPES: &[&str] = &[
    "string", "integer", "int", "i32", "i64", "u32", "u64", "isize", "usize",
    "number", "float", "f32", "f64", "boolean", "bool", "enum", "array", "list",
    "object", "dict", "map",
];

pub(crate) fn validate_tool_node(
    node: &KdlNode,
    input: &str,
    report: &mut DiagnosticReport,
    has_server_default_profile: bool,
) {
    let name_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "name"));
    let tool_name = name_opt.clone().unwrap_or_else(|| "unnamed_tool".to_string());
    let tool_offset = SourceLocation::find_in_source(input, &format!("\"{}\"", tool_name), None)
        .or_else(|| SourceLocation::find_node_name(input, "tool", None))
        .map(|l| l.offset);

    if name_opt.is_none() || tool_name.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "tool", tool_offset);
        let mut diag = Diagnostic::error("E0050", "Tool declaration is missing a name")
            .with_help("Provide a tool name, e.g.: tool \"my_tool\" { ... }");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing tool name");
        }
        report.add(diag);
    }

    let mut exec_count = 0;
    let mut profile_count = 0;
    let mut http_count = 0;
    let mut ipc_count = 0;
    let mut native_count = 0;
    let mut com_count = 0;
    let mut ws_count = 0;
    let mut pipe_count = 0;

    if get_prop(node, "profile").is_some() {
        profile_count += 1;
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_TOOL_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, tool_offset);
                let mut diag = Diagnostic::error("E0051", format!("Unknown property '{}' on tool '{}'", key, tool_name))
                    .with_help(format!("Allowed tool properties are: {}.", ALLOWED_TOOL_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_TOOL_PROPS.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                }
                report.add(diag);
            }
        }
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            let child_offset = SourceLocation::find_node_name(input, child_name, tool_offset).map(|l| l.offset);

            if !ALLOWED_TOOL_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, tool_offset);
                let mut diag = Diagnostic::error("E0052", format!("Unknown child node '{}' in tool '{}'", child_name, tool_name))
                    .with_help(format!("Allowed children in a 'tool' block are: {}.", ALLOWED_TOOL_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_TOOL_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
                continue;
            }

            match child_name {
                "param" => validate_param_node(child, input, &tool_name, child_offset, report),
                "output" => validate_output_node(child, input, &tool_name, child_offset, report),
                "profile" | "bind:profile" => {
                    profile_count += 1;
                    validate_tool_profile_node(child, input, &tool_name, child_offset, report);
                }
                "exec" | "bind:exec" | "exec:windows" | "bind:exec:windows" | "exec:linux" | "bind:exec:linux"
                | "exec:macos" | "bind:exec:macos" | "exec:darwin" | "bind:exec:darwin" | "exec:osx" | "bind:exec:osx"
                | "exec:unix" | "bind:exec:unix" | "exec:fallback" | "bind:exec:fallback" | "exec:default" | "bind:exec:default"
                | "fallback" | "default" => {
                    exec_count += 1;
                    validate_exec_binding_node(child, input, &tool_name, child_offset, report);
                }
                "http" | "bind:http" => {
                    http_count += 1;
                    validate_http_binding_node(child, input, &tool_name, child_offset, report);
                }
                "ipc" | "bind:ipc" => {
                    ipc_count += 1;
                    validate_ipc_binding_node(child, input, &tool_name, child_offset, report);
                }
                "native" | "bind:native" => {
                    native_count += 1;
                    validate_native_binding_node(child, input, &tool_name, child_offset, report);
                }
                "com" | "bind:com" => {
                    com_count += 1;
                    validate_com_binding_node(child, input, &tool_name, child_offset, report);
                }
                "ws" | "bind:ws" | "websocket" | "bind:websocket" => {
                    ws_count += 1;
                    validate_ws_binding_node(child, input, &tool_name, child_offset, report);
                }
                "pipe" | "bind:pipe" | "tcp" | "bind:tcp" => {
                    pipe_count += 1;
                    validate_pipe_binding_node(child, input, &tool_name, child_offset, report);
                }
                _ => {}
            }
        }
    }

    let total_binding_kinds = (if exec_count > 0 { 1 } else { 0 })
        + (if profile_count > 0 { 1 } else { 0 })
        + http_count + ipc_count + native_count + com_count + ws_count + pipe_count;

    if total_binding_kinds == 0 && !has_server_default_profile {
        let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", tool_name), tool_offset)
            .or_else(|| SourceLocation::find_node_name(input, "tool", tool_offset));
        let mut diag = Diagnostic::error("E0053", format!("Tool '{}' has no execution binding", tool_name))
            .with_help("Every tool must declare an execution binding (bind:exec, bind:http, bind:ipc, bind:native, bind:com, bind:ws, bind:pipe) or use a profile.");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing execution binding");
        }
        report.add(diag);
    } else if total_binding_kinds > 1 {
        let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", tool_name), tool_offset)
            .or_else(|| SourceLocation::find_node_name(input, "tool", tool_offset));
        let mut diag = Diagnostic::error(
            "E0054",
            format!("Tool '{}' declares multiple execution bindings ({})", tool_name, total_binding_kinds),
        )
        .with_help("A tool must have exactly one execution binding or profile.");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label(format!("declared {} conflicting bindings", total_binding_kinds));
        }
        report.add(diag);
    }
}

pub(crate) fn validate_param_node(
    node: &KdlNode,
    input: &str,
    parent_name: &str,
    param_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let name_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "name"));
    let param_name = name_opt.clone().unwrap_or_else(|| "unnamed_param".to_string());
    let p_offset = SourceLocation::find_in_source(input, &format!("\"{}\"", param_name), param_offset)
        .or_else(|| SourceLocation::find_node_name(input, "param", param_offset))
        .map(|l| l.offset);

    if name_opt.is_none() || param_name.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "param", param_offset);
        let mut diag = Diagnostic::error("E0070", format!("Parameter declaration on '{}' is missing a name", parent_name))
            .with_help("Specify a parameter name: param \"param_name\" type=\"string\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing parameter name");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_PARAM_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, p_offset);
                let mut diag = Diagnostic::error(
                    "E0070",
                    format!("Unknown property '{}' on parameter '{}' of '{}'", key, param_name, parent_name),
                )
                .with_help(format!("Allowed parameter properties are: {}.", ALLOWED_PARAM_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_PARAM_PROPS.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                }
                report.add(diag);
            }
        }
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            if !ALLOWED_PARAM_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, p_offset);
                let mut diag = Diagnostic::error(
                    "E0070",
                    format!("Unknown child node '{}' on parameter '{}' of '{}'", child_name, param_name, parent_name),
                )
                .with_help(format!("Allowed parameter children are: {}.", ALLOWED_PARAM_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_PARAM_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            }
        }
    }

    let mut raw_type = get_prop(node, "type");
    if raw_type.is_none() {
        if let Some(entry) = node.entries().first() {
            if let Some(ty) = entry.ty() {
                raw_type = Some(ty.value().to_string());
            }
        }
    }
    if raw_type.is_none() {
        if let Some(ty) = node.ty() {
            raw_type = Some(ty.value().to_string());
        }
    }
    if raw_type.is_none() {
        if let Some(children) = node.children() {
            for child in children.nodes() {
                if child.name().value() == "type" {
                    raw_type = get_arg_str(child, 0);
                    break;
                }
            }
        }
    }

    if let Some(ty_str) = &raw_type {
        let ty_lower = ty_str.to_lowercase();
        if !ALLOWED_PARAM_TYPES.contains(&ty_lower.as_str()) && !ty_lower.starts_with("array:") {
            let loc = SourceLocation::find_in_source(input, ty_str, p_offset);
            let mut diag = Diagnostic::error(
                "E0071",
                format!("Unknown parameter type '{}' on parameter '{}' of '{}'", ty_str, param_name, parent_name),
            )
            .with_help("Supported parameter types are: string, integer, number, boolean, array, object.");
            if let Some(closest) = find_closest_match(&ty_lower, ALLOWED_PARAM_TYPES.iter().copied()) {
                diag = diag.with_note(format!("Did you mean '{}'?", closest));
            }
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("unknown parameter type '{}'", ty_str));
            }
            report.add(diag);
        }
    }

    let mut default_val = get_prop_json(node, "default");
    if default_val.is_none() {
        if let Some(children) = node.children() {
            for child in children.nodes() {
                if child.name().value() == "default" {
                    default_val = get_arg_json(child, 0);
                    break;
                }
            }
        }
    }

    let mut enum_values = None;
    if let Some(enum_str) = get_prop(node, "enum") {
        let values: Vec<String> = enum_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !values.is_empty() {
            enum_values = Some(values);
        }
    } else if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "enum" {
                let mut values = Vec::new();
                for entry in child.entries() {
                    if entry.name().is_none() {
                        if let Some(s) = val_as_string(entry.value()) {
                            values.push(s);
                        }
                    }
                }
                if values.len() == 1 && values[0].contains(',') {
                    values = values[0]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                if !values.is_empty() {
                    enum_values = Some(values);
                }
                break;
            }
        }
    }

    if let (Some(def), Some(enums)) = (&default_val, &enum_values) {
        let def_str = match def {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !enums.contains(&def_str) {
            let loc = SourceLocation::find_in_source(input, &def_str, p_offset);
            let mut diag = Diagnostic::error(
                "E0073",
                format!(
                    "Default value '{}' for parameter '{}' of '{}' is not in allowed enum values [{}]",
                    def_str,
                    param_name,
                    parent_name,
                    enums.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ")
                ),
            )
            .with_help("Set default to one of the defined enum options.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("invalid default '{}'", def_str));
            }
            report.add(diag);
        }
    }
}

pub(crate) fn parse_tool_node(node: &KdlNode) -> Result<ToolSpec, ParserError> {
    let name = get_arg_str(node, 0).or_else(|| get_prop(node, "name")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "tool".to_string(),
            message: "Tool declaration is missing a name".to_string(),
        }
    })?;

    let mut description = get_prop(node, "description");
    let mut profile = get_prop(node, "profile").map(|p| ToolProfileRef {
        name: p,
        ..Default::default()
    });
    let mut params = Vec::new();
    let mut binding = None;
    let mut output = OutputSpec::default();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            match child_name {
                "description" => {
                    description = get_arg_str(child, 0);
                }
                "param" => {
                    let param = parse_param_node(child)?;
                    params.push(param);
                }
                "output" => {
                    output = parse_output_node(child)?;
                }
                "profile" | "bind:profile" => {
                    let p = parse_tool_profile_node(child)?;
                    profile = Some(p);
                }
                "exec:windows" | "bind:exec:windows" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Windows))?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => e.variants.push(variant),
                        _ => binding = Some(ToolBinding::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:linux" | "bind:exec:linux" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Linux))?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => e.variants.push(variant),
                        _ => binding = Some(ToolBinding::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:macos" | "bind:exec:macos" | "exec:darwin" | "bind:exec:darwin" | "exec:osx" | "bind:exec:osx" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Macos))?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => e.variants.push(variant),
                        _ => binding = Some(ToolBinding::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:unix" | "bind:exec:unix" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Unix))?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => e.variants.push(variant),
                        _ => binding = Some(ToolBinding::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:fallback" | "bind:exec:fallback" | "exec:default" | "bind:exec:default" | "fallback" | "default" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Fallback))?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => e.variants.push(variant),
                        _ => binding = Some(ToolBinding::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec" | "bind:exec" => {
                    let parsed = parse_exec_binding(child)?;
                    match binding {
                        Some(ToolBinding::Exec(ref mut e)) => {
                            if !parsed.command.is_empty() {
                                e.command = parsed.command;
                            }
                            if !parsed.args.is_empty() {
                                e.args = parsed.args;
                            }
                            if parsed.workdir.is_some() {
                                e.workdir = parsed.workdir;
                            }
                            if parsed.timeout_ms != 10_000 {
                                e.timeout_ms = parsed.timeout_ms;
                            }
                            e.variants.extend(parsed.variants);
                        }
                        _ => binding = Some(ToolBinding::Exec(parsed)),
                    }
                }
                "http" | "bind:http" => {
                    let http = parse_http_binding(child)?;
                    binding = Some(ToolBinding::Http(http));
                }
                "ipc" | "bind:ipc" => {
                    let ipc = parse_ipc_binding(child)?;
                    binding = Some(ToolBinding::Ipc(ipc));
                }
                "native" | "bind:native" => {
                    binding = Some(ToolBinding::Native(parse_native_binding(child, &name)?));
                }
                "com" | "bind:com" => {
                    let com = parse_com_binding(child)?;
                    binding = Some(ToolBinding::Com(com));
                }
                "ws" | "bind:ws" | "websocket" | "bind:websocket" => {
                    let ws = parse_ws_binding(child)?;
                    binding = Some(ToolBinding::Ws(ws));
                }
                "pipe" | "bind:pipe" | "tcp" | "bind:tcp" => {
                    let pipe = parse_pipe_binding(child)?;
                    binding = Some(ToolBinding::Pipe(pipe));
                }
                _ => {}
            }
        }
    }

    Ok(ToolSpec {
        name,
        description,
        profile,
        params,
        binding,
        output,
    })
}

pub(crate) fn parse_param_node(node: &KdlNode) -> Result<ParamSpec, ParserError> {
    let name = get_arg_str(node, 0).or_else(|| get_prop(node, "name")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "param".to_string(),
            message: "Parameter declaration missing name argument".to_string(),
        }
    })?;

    let mut raw_type = get_prop(node, "type");
    if raw_type.is_none() {
        if let Some(entry) = node.entries().first() {
            if let Some(ty) = entry.ty() {
                raw_type = Some(ty.value().to_string());
            }
        }
    }
    if raw_type.is_none() {
        if let Some(ty) = node.ty() {
            raw_type = Some(ty.value().to_string());
        }
    }

    let mut description = get_prop(node, "description");
    let required_prop = get_prop_bool(node, "required");
    let mut default_prop = get_prop_json(node, "default");
    let mut minimum = get_prop_f64(node, "minimum").or_else(|| get_prop_f64(node, "min"));
    let mut maximum = get_prop_f64(node, "maximum").or_else(|| get_prop_f64(node, "max"));
    let mut pattern = get_prop(node, "pattern");
    let mut enum_values = None;
    if let Some(enum_str) = get_prop(node, "enum") {
        let values: Vec<String> = enum_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !values.is_empty() {
            enum_values = Some(values);
        }
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "type" => {
                    if raw_type.is_none() {
                        raw_type = get_arg_str(child, 0);
                    }
                }
                "description" => {
                    description = get_arg_str(child, 0);
                }
                "default" => {
                    default_prop = get_arg_json(child, 0);
                }
                "minimum" | "min" => {
                    minimum = get_arg_f64(child, 0);
                }
                "maximum" | "max" => {
                    maximum = get_arg_f64(child, 0);
                }
                "pattern" => {
                    pattern = get_arg_str(child, 0);
                }
                "enum" => {
                    let mut values = Vec::new();
                    for entry in child.entries() {
                        if entry.name().is_none() {
                            if let Some(s) = val_as_string(entry.value()) {
                                values.push(s);
                            }
                        }
                    }
                    if values.len() == 1 && values[0].contains(',') {
                        values = values[0]
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    if !values.is_empty() {
                        enum_values = Some(values);
                    }
                }
                _ => {}
            }
        }
    }

    let param_type = match raw_type.as_deref().unwrap_or("string") {
        "integer" | "int" | "i32" | "i64" | "u32" | "u64" | "isize" | "usize" => ParamType::Integer,
        "number" | "float" | "f32" | "f64" => ParamType::Number,
        "boolean" | "bool" => ParamType::Boolean,
        "enum" => ParamType::String,
        "array" | "list" => ParamType::Array(Box::new(ParamType::String)),
        s if s.starts_with("array:") => {
            let inner = &s[6..];
            let inner_ty = match inner {
                "integer" | "int" | "i32" | "i64" => ParamType::Integer,
                "number" | "float" => ParamType::Number,
                "boolean" | "bool" => ParamType::Boolean,
                "object" => ParamType::Object,
                _ => ParamType::String,
            };
            ParamType::Array(Box::new(inner_ty))
        }
        "object" | "dict" | "map" => ParamType::Object,
        _ => ParamType::String,
    };

    let required = required_prop.unwrap_or(default_prop.is_none());

    Ok(ParamSpec {
        name,
        param_type,
        description,
        required,
        default: default_prop,
        minimum,
        maximum,
        pattern,
        enum_values,
    })
}
