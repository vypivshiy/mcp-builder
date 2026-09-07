use crate::ast::IpcBinding;
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_IPC_PROPS: &[&str] = &[
    "dir",
    "directory",
    "path",
    "method",
    "params",
    "payload",
    "body",
    "timeout-ms",
    "timeout_ms",
    "timeout",
];

pub const ALLOWED_IPC_CHILDREN: &[&str] = &[
    "dir",
    "directory",
    "path",
    "method",
    "params",
    "payload",
    "body",
    "timeout-ms",
    "timeout_ms",
    "timeout",
];

pub fn validate_ipc_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let mut dir = get_prop(node, "dir")
        .or_else(|| get_prop(node, "directory"))
        .or_else(|| get_prop(node, "path"))
        .or_else(|| get_arg_str(node, 0));
    let mut method = get_prop(node, "method");

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            if !ALLOWED_IPC_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error("E0060", format!("Unknown child node '{}' in ipc binding on tool '{}'", child_name, tool_name))
                    .with_help(format!("Allowed ipc options are: {}.", ALLOWED_IPC_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_IPC_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            }
            match child_name {
                "dir" | "directory" | "path" => {
                    if let Some(d) = get_arg_str(child, 0).or_else(|| get_prop(child, "path")) {
                        dir = Some(d);
                    }
                }
                "method" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        method = Some(m);
                    }
                }
                _ => {}
            }
        }
    }

    if dir.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error("E0060", format!("IPC binding on tool '{}' is missing required 'dir' configuration", tool_name))
            .with_help("Specify IPC directory: dir \"path/to/ipc\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing dir");
        }
        report.add(diag);
    }

    if method.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error("E0060", format!("IPC binding on tool '{}' is missing required 'method' configuration", tool_name))
            .with_help("Specify IPC method: method \"method_name\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing method");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_IPC_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error("E0060", format!("Unknown property '{}' in ipc binding on tool '{}'", key, tool_name))
                    .with_help(format!("Allowed ipc properties are: {}.", ALLOWED_IPC_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_IPC_PROPS.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                }
                report.add(diag);
            }
        }
    }
}

pub fn parse_ipc_binding(node: &KdlNode) -> Result<IpcBinding, ParserError> {
    let mut dir = get_prop(node, "dir")
        .or_else(|| get_prop(node, "directory"))
        .or_else(|| get_prop(node, "path"))
        .or_else(|| get_arg_str(node, 0))
        .unwrap_or_default();
    let mut method = get_prop(node, "method").unwrap_or_default();
    let mut params = get_prop(node, "params")
        .or_else(|| get_prop(node, "payload"))
        .or_else(|| get_prop(node, "body"));
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout"))
        .unwrap_or(10_000);

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "dir" | "directory" | "path" => {
                    if let Some(d) = get_arg_str(child, 0).or_else(|| get_prop(child, "path")) {
                        dir = d;
                    }
                }
                "method" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        method = m;
                    }
                }
                "params" | "payload" | "body" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        params = Some(p);
                    }
                }
                "timeout-ms" | "timeout" => {
                    if let Some(t) = get_arg_u64(child, 0) {
                        timeout_ms = t;
                    }
                }
                _ => {}
            }
        }
    }

    if dir.is_empty() {
        return Err(ParserError::ValidationError {
            location: "bind:ipc".to_string(),
            message: "IPC binding missing required 'dir' configuration".to_string(),
        });
    }

    if method.is_empty() {
        return Err(ParserError::ValidationError {
            location: "bind:ipc".to_string(),
            message: "IPC binding missing required 'method' configuration".to_string(),
        });
    }

    Ok(IpcBinding {
        dir,
        method,
        params,
        timeout_ms,
    })
}
