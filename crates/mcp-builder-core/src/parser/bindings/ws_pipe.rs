use crate::ast::{PipeBinding, WsBinding};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_WS_PROPS: &[&str] = &[
    "url",
    "host",
    "port",
    "endpoint",
    "path",
    "message",
    "body",
    "payload",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub const ALLOWED_WS_CHILDREN: &[&str] = &[
    "url",
    "host",
    "port",
    "endpoint",
    "path",
    "message",
    "body",
    "payload",
    "header",
    "headers",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub const ALLOWED_PIPE_PROPS: &[&str] = &[
    "host",
    "port",
    "addr",
    "address",
    "message",
    "payload",
    "body",
    "send",
    "framing",
    "delimiter",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub const ALLOWED_PIPE_CHILDREN: &[&str] = &[
    "host",
    "port",
    "addr",
    "address",
    "message",
    "payload",
    "body",
    "send",
    "framing",
    "delimiter",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub fn validate_ws_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let url_opt = get_prop(node, "url").or_else(|| get_arg_str(node, 0));
    let host_opt = get_prop(node, "host");
    let mut has_child_url_or_host = false;
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "url" || child.name().value() == "host" {
                has_child_url_or_host = true;
                break;
            }
        }
    }

    if !has_child_url_or_host
        && url_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true)
        && host_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true)
    {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error(
            "E0063",
            format!("WebSocket binding on tool '{}' is missing a target 'url' or 'host'", tool_name),
        )
        .with_help("Specify a target WebSocket URL, e.g.: bind:ws url=\"ws://127.0.0.1:9222/devtools/page/{target_id}\" or bind:ws host=\"127.0.0.1\" port=9222");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing target url or host");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_WS_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error(
                    "E0063",
                    format!("Unknown property '{}' in ws binding on tool '{}'", key, tool_name),
                )
                .with_help(format!("Allowed ws properties are: {}.", ALLOWED_WS_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_WS_PROPS.iter().copied()) {
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
            if !ALLOWED_WS_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error(
                    "E0063",
                    format!("Unknown child node '{}' in ws binding on tool '{}'", child_name, tool_name),
                )
                .with_help(format!("Allowed ws child nodes are: {}.", ALLOWED_WS_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_WS_CHILDREN.iter().copied()) {
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

pub fn validate_pipe_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let addr_opt = get_prop(node, "addr").or_else(|| get_prop(node, "address")).or_else(|| get_arg_str(node, 0));
    let host_opt = get_prop(node, "host");
    let port_opt = get_prop(node, "port");
    let mut has_child_target = false;
    if let Some(children) = node.children() {
        for child in children.nodes() {
            let c_name = child.name().value();
            if c_name == "addr" || c_name == "address" || c_name == "host" || c_name == "port" {
                has_child_target = true;
                break;
            }
        }
    }

    if !has_child_target
        && addr_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true)
        && (host_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) && port_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true))
    {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error(
            "E0064",
            format!("Pipe binding on tool '{}' is missing a target address ('addr' or 'host'/'port')", tool_name),
        )
        .with_help("Specify a target address: bind:pipe \"127.0.0.1:6379\" or bind:pipe host=\"127.0.0.1\" port=6379");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing target address");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_PIPE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error(
                    "E0064",
                    format!("Unknown property '{}' in pipe binding on tool '{}'", key, tool_name),
                )
                .with_help(format!("Allowed pipe properties are: {}.", ALLOWED_PIPE_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_PIPE_PROPS.iter().copied()) {
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
            if !ALLOWED_PIPE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error(
                    "E0064",
                    format!("Unknown child node '{}' in pipe binding on tool '{}'", child_name, tool_name),
                )
                .with_help(format!("Allowed pipe child nodes are: {}.", ALLOWED_PIPE_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_PIPE_CHILDREN.iter().copied()) {
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

pub fn parse_ws_binding(node: &KdlNode) -> Result<WsBinding, ParserError> {
    let mut url = get_prop(node, "url").or_else(|| get_arg_str(node, 0));
    let mut host = get_prop(node, "host");
    let mut port = get_prop(node, "port");
    let mut endpoint = get_prop(node, "endpoint").or_else(|| get_prop(node, "path"));
    let mut message = get_prop(node, "message")
        .or_else(|| get_prop(node, "body"))
        .or_else(|| get_prop(node, "payload"))
        .unwrap_or_default();
    let mut headers = Vec::new();
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout_ms"))
        .or_else(|| get_prop_u64(node, "timeout"))
        .unwrap_or(10_000);
    let mut extract_json = get_prop(node, "extract-json")
        .or_else(|| get_prop(node, "extract_json"))
        .or_else(|| get_prop(node, "extract"));

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "url" => {
                    if let Some(u) = get_arg_str(child, 0) {
                        url = Some(u);
                    }
                }
                "host" => host = get_arg_str(child, 0),
                "port" => {
                    if let Some(p) = get_arg_str(child, 0).or_else(|| get_arg_u64(child, 0).map(|v| v.to_string())) {
                        port = Some(p);
                    }
                }
                "endpoint" | "path" => endpoint = get_arg_str(child, 0),
                "message" | "body" | "payload" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        message = m;
                    }
                }
                "header" => {
                    if let (Some(k), Some(v)) = (get_arg_str(child, 0), get_arg_str(child, 1)) {
                        headers.push((k, v));
                    }
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(t) = get_arg_u64(child, 0) {
                        timeout_ms = t;
                    }
                }
                "extract-json" | "extract_json" | "extract" => {
                    extract_json = get_arg_str(child, 0);
                }
                _ => {}
            }
        }
    }

    Ok(WsBinding {
        url,
        host,
        port,
        endpoint,
        headers,
        message,
        timeout_ms,
        extract_json,
    })
}

pub fn parse_pipe_binding(node: &KdlNode) -> Result<PipeBinding, ParserError> {
    let mut host = get_prop(node, "host");
    let mut port = get_prop(node, "port");
    let mut addr = get_prop(node, "addr")
        .or_else(|| get_prop(node, "address"))
        .or_else(|| get_arg_str(node, 0));
    let mut message = get_prop(node, "message")
        .or_else(|| get_prop(node, "payload"))
        .or_else(|| get_prop(node, "body"))
        .or_else(|| get_prop(node, "send"))
        .unwrap_or_default();
    let mut framing = get_prop(node, "framing")
        .or_else(|| get_prop(node, "delimiter"))
        .unwrap_or_else(|| "\n".to_string());
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout_ms"))
        .or_else(|| get_prop_u64(node, "timeout"))
        .unwrap_or(10_000);
    let mut extract_json = get_prop(node, "extract-json")
        .or_else(|| get_prop(node, "extract_json"))
        .or_else(|| get_prop(node, "extract"));

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "host" => host = get_arg_str(child, 0),
                "port" => {
                    if let Some(p) = get_arg_str(child, 0).or_else(|| get_arg_u64(child, 0).map(|v| v.to_string())) {
                        port = Some(p);
                    }
                }
                "addr" | "address" => addr = get_arg_str(child, 0),
                "message" | "payload" | "body" | "send" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        message = m;
                    }
                }
                "framing" | "delimiter" => {
                    if let Some(f) = get_arg_str(child, 0) {
                        framing = f;
                    }
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(t) = get_arg_u64(child, 0) {
                        timeout_ms = t;
                    }
                }
                "extract-json" | "extract_json" | "extract" => {
                    extract_json = get_arg_str(child, 0);
                }
                _ => {}
            }
        }
    }

    Ok(PipeBinding {
        host,
        port,
        addr,
        message,
        framing,
        timeout_ms,
        extract_json,
    })
}
