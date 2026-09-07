use crate::ast::NativeBinding;
use crate::diagnostics::{Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::get_prop;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_NATIVE_PROPS: &[&str] = &["symbol"];
pub const ALLOWED_NATIVE_CHILDREN: &[&str] = &["symbol"];

pub fn validate_native_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_NATIVE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error("E0061", format!("Unknown property '{}' in native binding on tool '{}'", key, tool_name))
                    .with_help("Allowed native binding property is 'symbol'.");
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
            if !ALLOWED_NATIVE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error("E0061", format!("Unknown child node '{}' in native binding on tool '{}'", child_name, tool_name))
                    .with_help("Allowed native binding child node is 'symbol'.");
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            }
        }
    }
}

pub fn parse_native_binding(node: &KdlNode, fallback_name: &str) -> Result<NativeBinding, ParserError> {
    let symbol = get_prop(node, "symbol")
        .or_else(|| crate::helpers::get_arg_str(node, 0))
        .unwrap_or_else(|| format!("handle_{}", fallback_name));
    Ok(NativeBinding { symbol })
}
