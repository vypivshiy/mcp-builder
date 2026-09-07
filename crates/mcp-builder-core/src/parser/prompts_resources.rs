use crate::ast::{
    ExecBinding, PromptArgSpec, PromptMessageSpec, PromptSpec, ResourceContent, ResourceSpec,
    ResourceTemplateSpec, TargetOs, ToolBinding, ToolProfileRef,
};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::bindings::{parse_exec_binding, parse_exec_variant};
use crate::parser::profiles::parse_tool_profile_node;
use crate::parser::tools::{parse_param_node, validate_param_node};
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_RESOURCE_PROPS: &[&str] = &["uri", "name", "description", "mime-type", "mime_type"];

pub const ALLOWED_RESOURCE_CHILDREN: &[&str] = &[
    "name",
    "description",
    "mime-type",
    "mime_type",
    "text",
    "binary",
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
    "native",
    "bind:native",
];

pub const ALLOWED_RESOURCE_TEMPLATE_PROPS: &[&str] = &[
    "uri",
    "name",
    "description",
    "mime-type",
    "mime_type",
];

pub const ALLOWED_RESOURCE_TEMPLATE_CHILDREN: &[&str] = &[
    "name",
    "description",
    "mime-type",
    "mime_type",
    "param",
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
];

pub const ALLOWED_PROMPT_PROPS: &[&str] = &["name", "description"];
pub const ALLOWED_PROMPT_CHILDREN: &[&str] = &["description", "argument", "message"];
pub const ALLOWED_PROMPT_ARG_PROPS: &[&str] = &["name", "required", "description", "default"];
pub const ALLOWED_PROMPT_ARG_CHILDREN: &[&str] = &["required", "description", "default"];
pub const ALLOWED_PROMPT_MSG_PROPS: &[&str] = &["role", "content"];
pub const ALLOWED_PROMPT_MSG_CHILDREN: &[&str] = &["role", "content"];
pub const ALLOWED_MSG_ROLES: &[&str] = &["user", "assistant", "system"];

pub(crate) fn validate_resource_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let uri_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "uri"));
    let res_uri = uri_opt.clone().unwrap_or_else(|| "unnamed_resource".to_string());
    let res_offset = SourceLocation::find_in_source(input, &res_uri, None).map(|l| l.offset);

    if uri_opt.is_none() || res_uri.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "resource", res_offset);
        let mut diag = Diagnostic::error("E0080", "Resource declaration is missing a URI")
            .with_help("Specify a URI: resource \"system://info\" { ... }");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing URI");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_RESOURCE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, res_offset);
                let mut diag = Diagnostic::error("E0081", format!("Unknown property '{}' on resource '{}'", key, res_uri))
                    .with_help(format!("Allowed resource properties are: {}.", ALLOWED_RESOURCE_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_RESOURCE_PROPS.iter().copied()) {
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
            if !ALLOWED_RESOURCE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, res_offset);
                let mut diag = Diagnostic::error("E0081", format!("Unknown child node '{}' on resource '{}'", child_name, res_uri))
                    .with_help(format!("Allowed resource children are: {}.", ALLOWED_RESOURCE_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_RESOURCE_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            }
        }
    }
}

pub(crate) fn validate_resource_template_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let uri_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "uri"));
    let res_uri = uri_opt.clone().unwrap_or_else(|| "unnamed_template".to_string());
    let res_offset = SourceLocation::find_in_source(input, &res_uri, None).map(|l| l.offset);

    if uri_opt.is_none() || res_uri.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "resource-template", res_offset);
        let mut diag = Diagnostic::error("E0085", "Resource template is missing a URI template")
            .with_help("Specify a URI template: resource-template \"db://{table}/schema\" { ... }");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing URI template");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_RESOURCE_TEMPLATE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, res_offset);
                let mut diag = Diagnostic::error("E0086", format!("Unknown property '{}' on resource-template '{}'", key, res_uri))
                    .with_help(format!("Allowed resource-template properties are: {}.", ALLOWED_RESOURCE_TEMPLATE_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_RESOURCE_TEMPLATE_PROPS.iter().copied()) {
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
            let child_offset = SourceLocation::find_node_name(input, child_name, res_offset).map(|l| l.offset);
            if !ALLOWED_RESOURCE_TEMPLATE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, res_offset);
                let mut diag = Diagnostic::error("E0086", format!("Unknown child node '{}' on resource-template '{}'", child_name, res_uri))
                    .with_help(format!("Allowed resource-template children are: {}.", ALLOWED_RESOURCE_TEMPLATE_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_RESOURCE_TEMPLATE_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            } else if child_name == "param" {
                validate_param_node(child, input, &res_uri, child_offset, report);
            }
        }
    }
}

pub(crate) fn validate_prompt_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let name_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "name"));
    let prompt_name = name_opt.clone().unwrap_or_else(|| "unnamed_prompt".to_string());
    let prompt_offset = SourceLocation::find_in_source(input, &format!("\"{}\"", prompt_name), None).map(|l| l.offset);

    if name_opt.is_none() || prompt_name.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "prompt", prompt_offset);
        let mut diag = Diagnostic::error("E0090", "Prompt declaration is missing a name")
            .with_help("Specify a prompt name: prompt \"prompt_name\" { ... }");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing prompt name");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_PROMPT_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, prompt_offset);
                let mut diag = Diagnostic::error("E0091", format!("Unknown property '{}' on prompt '{}'", key, prompt_name))
                    .with_help(format!("Allowed prompt properties are: {}.", ALLOWED_PROMPT_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_PROMPT_PROPS.iter().copied()) {
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
            let child_offset = SourceLocation::find_node_name(input, child_name, prompt_offset).map(|l| l.offset);
            if !ALLOWED_PROMPT_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, prompt_offset);
                let mut diag = Diagnostic::error("E0091", format!("Unknown child node '{}' in prompt '{}'", child_name, prompt_name))
                    .with_help(format!("Allowed prompt children are: {}.", ALLOWED_PROMPT_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_PROMPT_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            } else if child_name == "argument" {
                for entry in child.entries() {
                    if let Some(prop_name) = entry.name() {
                        let key = prop_name.value();
                        if !ALLOWED_PROMPT_ARG_PROPS.contains(&key) {
                            let loc = SourceLocation::find_property(input, key, child_offset);
                            let mut diag = Diagnostic::error("E0092", format!("Unknown property '{}' on prompt argument in '{}'", key, prompt_name))
                                .with_help(format!("Allowed argument properties are: {}.", ALLOWED_PROMPT_ARG_PROPS.join(", ")));
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                            }
                            report.add(diag);
                        }
                    }
                }
                if let Some(arg_children) = child.children() {
                    for ac in arg_children.nodes() {
                        let ac_name = ac.name().value();
                        if !ALLOWED_PROMPT_ARG_CHILDREN.contains(&ac_name) {
                            let loc = SourceLocation::find_node_name(input, ac_name, child_offset);
                            let mut diag = Diagnostic::error("E0092", format!("Unknown child node '{}' in prompt argument in '{}'", ac_name, prompt_name))
                                .with_help(format!("Allowed argument options are: {}.", ALLOWED_PROMPT_ARG_CHILDREN.join(", ")));
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label(format!("unknown child node '{}'", ac_name));
                            }
                            report.add(diag);
                        }
                    }
                }
            } else if child_name == "message" {
                if let Some(role) = get_prop(child, "role") {
                    if !ALLOWED_MSG_ROLES.contains(&role.to_lowercase().as_str()) {
                        let loc = SourceLocation::find_property(input, "role", child_offset);
                        let mut diag = Diagnostic::error("E0093", format!("Invalid message role '{}' in prompt '{}'", role, prompt_name))
                            .with_help("Allowed message roles are: user, assistant, system.");
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("invalid role '{}'", role));
                        }
                        report.add(diag);
                    }
                }
                for entry in child.entries() {
                    if let Some(prop_name) = entry.name() {
                        let key = prop_name.value();
                        if !ALLOWED_PROMPT_MSG_PROPS.contains(&key) {
                            let loc = SourceLocation::find_property(input, key, child_offset);
                            let mut diag = Diagnostic::error("E0094", format!("Unknown property '{}' on prompt message in '{}'", key, prompt_name))
                                .with_help(format!("Allowed message properties are: {}.", ALLOWED_PROMPT_MSG_PROPS.join(", ")));
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                            }
                            report.add(diag);
                        }
                    }
                }
                if let Some(msg_children) = child.children() {
                    for mc in msg_children.nodes() {
                        let mc_name = mc.name().value();
                        if !ALLOWED_PROMPT_MSG_CHILDREN.contains(&mc_name) {
                            let loc = SourceLocation::find_node_name(input, mc_name, child_offset);
                            let mut diag = Diagnostic::error("E0094", format!("Unknown child node '{}' in prompt message in '{}'", mc_name, prompt_name))
                                .with_help(format!("Allowed message options are: {}.", ALLOWED_PROMPT_MSG_CHILDREN.join(", ")));
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label(format!("unknown child node '{}'", mc_name));
                            }
                            report.add(diag);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn parse_resource_node(node: &KdlNode) -> Result<ResourceSpec, ParserError> {
    let uri = get_arg_str(node, 0).or_else(|| get_prop(node, "uri")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "resource".to_string(),
            message: "Resource declaration missing URI argument".to_string(),
        }
    })?;

    let mut name = get_prop(node, "name");
    let mut description = get_prop(node, "description");
    let mut mime_type = get_prop(node, "mime-type").or_else(|| get_prop(node, "mime_type"));
    let mut content = None;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "name" => name = get_arg_str(child, 0),
                "description" => description = get_arg_str(child, 0),
                "mime-type" | "mime_type" => mime_type = get_arg_str(child, 0),
                "text" => {
                    if let Some(t) = get_arg_str(child, 0) {
                        content = Some(ResourceContent::Text { text: t });
                    }
                }
                "binary" => {
                    if let Some(b) = get_arg_str(child, 0) {
                        content = Some(ResourceContent::Binary { blob: b });
                    }
                }
                "exec:windows" | "bind:exec:windows" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Windows))?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => e.variants.push(variant),
                        _ => content = Some(ResourceContent::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:linux" | "bind:exec:linux" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Linux))?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => e.variants.push(variant),
                        _ => content = Some(ResourceContent::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:macos" | "bind:exec:macos" | "exec:darwin" | "bind:exec:darwin" | "exec:osx" | "bind:exec:osx" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Macos))?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => e.variants.push(variant),
                        _ => content = Some(ResourceContent::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:unix" | "bind:exec:unix" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Unix))?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => e.variants.push(variant),
                        _ => content = Some(ResourceContent::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec:fallback" | "bind:exec:fallback" | "exec:default" | "bind:exec:default" | "fallback" | "default" => {
                    let variant = parse_exec_variant(child, Some(TargetOs::Fallback))?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => e.variants.push(variant),
                        _ => content = Some(ResourceContent::Exec(ExecBinding {
                            variants: vec![variant],
                            ..Default::default()
                        })),
                    }
                }
                "exec" | "bind:exec" => {
                    let parsed = parse_exec_binding(child)?;
                    match content {
                        Some(ResourceContent::Exec(ref mut e)) => {
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
                        _ => content = Some(ResourceContent::Exec(parsed)),
                    }
                }
                "native" | "bind:native" => {
                    let sym = get_prop(child, "symbol").or_else(|| get_arg_str(child, 0)).unwrap_or_default();
                    content = Some(ResourceContent::Native { symbol: sym });
                }
                _ => {}
            }
        }
    }

    Ok(ResourceSpec {
        uri,
        name,
        description,
        mime_type,
        content,
    })
}

pub(crate) fn parse_resource_template_node(node: &KdlNode) -> Result<ResourceTemplateSpec, ParserError> {
    let uri_template = get_arg_str(node, 0).or_else(|| get_prop(node, "uri")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "resource-template".to_string(),
            message: "Resource template missing URI template argument".to_string(),
        }
    })?;

    let mut name = get_prop(node, "name");
    let mut description = get_prop(node, "description");
    let mut mime_type = get_prop(node, "mime-type").or_else(|| get_prop(node, "mime_type"));
    let mut profile = get_prop(node, "profile").map(|p| ToolProfileRef {
        name: p,
        ..Default::default()
    });
    let mut params = Vec::new();
    let mut binding = None;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "name" => name = get_arg_str(child, 0),
                "description" => description = get_arg_str(child, 0),
                "mime-type" | "mime_type" => mime_type = get_arg_str(child, 0),
                "param" => {
                    params.push(parse_param_node(child)?);
                }
                "profile" | "bind:profile" => {
                    profile = Some(parse_tool_profile_node(child)?);
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
                _ => {}
            }
        }
    }

    Ok(ResourceTemplateSpec {
        uri_template,
        name,
        description,
        mime_type,
        profile,
        params,
        binding,
    })
}

pub(crate) fn parse_prompt_node(node: &KdlNode) -> Result<PromptSpec, ParserError> {
    let name = get_arg_str(node, 0).or_else(|| get_prop(node, "name")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "prompt".to_string(),
            message: "Prompt declaration missing name argument".to_string(),
        }
    })?;

    let mut description = get_prop(node, "description");
    let mut arguments = Vec::new();
    let mut messages = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "description" => description = get_arg_str(child, 0),
                "argument" => {
                    let arg_name = get_arg_str(child, 0).or_else(|| get_prop(child, "name")).unwrap_or_default();
                    let req = get_prop_bool(child, "required").unwrap_or(true);
                    let desc = get_prop(child, "description");
                    let def = get_prop(child, "default");
                    arguments.push(PromptArgSpec {
                        name: arg_name,
                        required: req,
                        description: desc,
                        default: def,
                    });
                }
                "message" => {
                    let role = get_prop(child, "role").unwrap_or_else(|| "user".to_string());
                    let content = get_prop(child, "content").or_else(|| get_arg_str(child, 0)).unwrap_or_default();
                    messages.push(PromptMessageSpec { role, content });
                }
                _ => {}
            }
        }
    }

    Ok(PromptSpec {
        name,
        description,
        arguments,
        messages,
    })
}
