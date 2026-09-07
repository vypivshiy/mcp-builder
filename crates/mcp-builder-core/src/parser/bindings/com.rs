use crate::ast::ComBinding;
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_COM_PROPS: &[&str] = &[
    "progid",
    "method",
    "dispatch",
    "call",
    "attach",
    "bring-to-front",
    "bring_to_front",
    "timeout-ms",
    "timeout_ms",
];

pub const ALLOWED_COM_CHILDREN: &[&str] = &[
    "progid",
    "dispatch",
    "call",
    "method",
    "get",
    "args",
    "attach",
    "bring-to-front",
    "bring_to_front",
    "timeout-ms",
    "timeout_ms",
];

pub fn validate_com_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let progid_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "progid"));
    let mut has_child_progid = false;
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "progid" {
                has_child_progid = true;
                break;
            }
        }
    }
    if !has_child_progid && progid_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        let loc = SourceLocation::find_node_name(input, node.name().value(), binding_offset);
        let mut diag = Diagnostic::error("E0061", format!("COM binding on tool '{}' is missing a progid", tool_name))
            .with_help("Specify a ProgID: bind:com \"Excel.Application\" or bind:com progid=\"Excel.Application\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing progid");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_COM_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error("E0062", format!("Unknown property '{}' in com binding on tool '{}'", key, tool_name))
                    .with_help(format!("Allowed com properties are: {}.", ALLOWED_COM_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_COM_PROPS.iter().copied()) {
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
            if !ALLOWED_COM_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error("E0062", format!("Unknown child node '{}' in com binding on tool '{}'", child_name, tool_name))
                    .with_help(format!("Allowed com child nodes are: {}.", ALLOWED_COM_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_COM_CHILDREN.iter().copied()) {
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

pub fn parse_com_binding(node: &KdlNode) -> Result<ComBinding, ParserError> {
    let mut progid = get_prop(node, "progid")
        .or_else(|| get_arg_str(node, 0))
        .unwrap_or_default();
    let mut method = get_prop(node, "method")
        .or_else(|| get_prop(node, "dispatch"))
        .or_else(|| get_prop(node, "call"))
        .unwrap_or_default();
    let mut args = Vec::new();
    let mut attach = get_prop(node, "attach")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(true);
    let mut bring_to_front = get_prop(node, "bring-to-front")
        .or_else(|| get_prop(node, "bring_to_front"))
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);
    let mut timeout_ms = get_prop(node, "timeout-ms")
        .or_else(|| get_prop(node, "timeout_ms"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(10_000);

    if let Some(c_children) = node.children() {
        for cc in c_children.nodes() {
            match cc.name().value() {
                "progid" => {
                    if let Some(p) = get_arg_str(cc, 0).or_else(|| get_prop(cc, "name")) {
                        progid = p;
                    }
                }
                "dispatch" | "call" | "method" | "get" => {
                    if let Some(m) = get_arg_str(cc, 0).or_else(|| get_prop(cc, "name")) {
                        method = m;
                    }
                    if let Some(a) = get_prop(cc, "args") {
                        args.push(a);
                    }
                    for entry in cc.entries().iter().skip(1) {
                        if let Some(val) = entry.value().as_string() {
                            args.push(val.to_string());
                        }
                    }
                }
                "args" => {
                    for entry in cc.entries() {
                        if let Some(val) = entry.value().as_string() {
                            args.push(val.to_string());
                        }
                    }
                }
                "attach" => {
                    attach = get_arg_bool(cc, 0).unwrap_or(true);
                }
                "bring-to-front" | "bring_to_front" => {
                    bring_to_front = get_arg_bool(cc, 0).unwrap_or(false);
                }
                "timeout-ms" | "timeout_ms" => {
                    if let Some(ms) = get_arg_u64(cc, 0).or_else(|| get_prop(cc, "ms").and_then(|s| s.parse().ok())) {
                        timeout_ms = ms;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(ComBinding {
        progid,
        method,
        args,
        attach,
        bring_to_front,
        timeout_ms,
    })
}
