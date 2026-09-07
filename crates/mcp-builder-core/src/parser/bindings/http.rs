use crate::ast::HttpBinding;
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_HTTP_PROPS: &[&str] = &[
    "method",
    "url",
    "body",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub const ALLOWED_HTTP_CHILDREN: &[&str] = &[
    "url",
    "method",
    "header",
    "headers",
    "query",
    "body",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "extract-json",
    "extract_json",
    "extract",
];

pub const ALLOWED_HTTP_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS",
];

pub fn validate_http_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let url_opt = get_prop(node, "url").or_else(|| get_arg_str(node, 0));
    if url_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error("E0057", format!("HTTP binding on tool '{}' is missing a URL", tool_name))
            .with_help("Specify a URL: bind:http url=\"https://...\" or bind:http \"https://...\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing URL");
        }
        report.add(diag);
    }

    if let Some(method_str) = get_prop(node, "method") {
        if !ALLOWED_HTTP_METHODS.contains(&method_str.to_uppercase().as_str()) {
            let loc = SourceLocation::find_property(input, "method", binding_offset);
            let mut diag = Diagnostic::error("E0058", format!("Invalid HTTP method '{}' on tool '{}'", method_str, tool_name))
                .with_help(format!("Allowed HTTP methods are: {}.", ALLOWED_HTTP_METHODS.join(", ")));
            if let Some(closest) = find_closest_match(&method_str.to_uppercase(), ALLOWED_HTTP_METHODS.iter().copied()) {
                diag = diag.with_note(format!("Did you mean '{}'?", closest));
            }
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("invalid HTTP method '{}'", method_str));
            }
            report.add(diag);
        }
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_HTTP_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error("E0059", format!("Unknown property '{}' in http binding on tool '{}'", key, tool_name))
                    .with_help(format!("Allowed http properties are: {}.", ALLOWED_HTTP_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_HTTP_PROPS.iter().copied()) {
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
            if !ALLOWED_HTTP_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error("E0059", format!("Unknown child node '{}' in http binding on tool '{}'", child_name, tool_name))
                    .with_help(format!("Allowed http children are: {}.", ALLOWED_HTTP_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_HTTP_CHILDREN.iter().copied()) {
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

pub fn parse_http_binding(node: &KdlNode) -> Result<HttpBinding, ParserError> {
    let mut method = get_prop(node, "method").unwrap_or_else(|| "GET".to_string());
    let mut url = get_prop(node, "url")
        .or_else(|| get_arg_str(node, 0))
        .unwrap_or_default();

    let mut headers = Vec::new();
    let mut query = Vec::new();
    let mut body = get_prop(node, "body");
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout"))
        .unwrap_or(10_000);
    let mut extract_json = get_prop(node, "extract-json").or_else(|| get_prop(node, "extract"));

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "url" => {
                    if let Some(u) = get_arg_str(child, 0) {
                        url = u;
                    }
                }
                "method" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        method = m;
                    }
                }
                "headers" => {
                    if let Some(h_children) = child.children() {
                        for hc in h_children.nodes() {
                            let h_name = hc.name().value().to_string();
                            let h_val = get_arg_str(hc, 0).unwrap_or_default();
                            headers.push((h_name, h_val));
                        }
                    }
                }
                "header" => {
                    if let (Some(k), Some(v)) = (get_arg_str(child, 0), get_arg_str(child, 1)) {
                        headers.push((k, v));
                    }
                }
                "query" => {
                    if let (Some(k), Some(v)) = (get_arg_str(child, 0), get_arg_str(child, 1)) {
                        query.push((k, v));
                    }
                }
                "body" => {
                    body = get_arg_str(child, 0);
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

    Ok(HttpBinding {
        method,
        url,
        headers,
        query,
        body,
        timeout_ms,
        extract_json,
    })
}
