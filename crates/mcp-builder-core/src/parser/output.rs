use crate::ast::{OutputFormat, OutputRegexSpec, OutputSliceSpec, OutputSpec};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_OUTPUT_PROPS: &[&str] = &[
    "format",
    "mime-type",
    "mime_type",
    "trim",
    "extract-json",
    "extract_json",
    "filter-not",
    "filter_not",
    "filter",
];

pub const ALLOWED_OUTPUT_CHILDREN: &[&str] = &[
    "format",
    "mime-type",
    "mime_type",
    "trim",
    "slice",
    "extract-json",
    "extract_json",
    "filter-not",
    "filter_not",
    "filter",
    "regex",
];

pub const ALLOWED_OUTPUT_FORMATS: &[&str] = &["text", "image", "img", "png", "jpeg", "json"];
pub const ALLOWED_SLICE_PROPS: &[&str] = &["lines", "head", "tail"];
pub const ALLOWED_REGEX_PROPS: &[&str] = &["pattern", "template"];
pub const ALLOWED_REGEX_CHILDREN: &[&str] = &["pattern", "template"];

pub(crate) fn validate_output_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    out_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_OUTPUT_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, out_offset);
                let mut diag = Diagnostic::error(
                    "E0076",
                    format!("Unknown property '{}' in output configuration of tool '{}'", key, tool_name),
                )
                .with_help(format!("Allowed output properties are: {}.", ALLOWED_OUTPUT_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_OUTPUT_PROPS.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                }
                report.add(diag);
            } else if key == "filter-not" || key == "filter_not" || key == "filter" {
                if let Some(pat_str) = entry.value().as_string() {
                    if let Err(e) = validate_regex_pattern(pat_str) {
                        let loc = SourceLocation::find_in_source(input, pat_str, out_offset);
                        let mut diag = Diagnostic::error(
                            "E0078",
                            format!("Invalid filter pattern '{}' in output configuration of tool '{}': {}", pat_str, tool_name, e),
                        )
                        .with_help("Ensure the filter pattern is a valid regular expression.");
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label("invalid regex syntax");
                        }
                        report.add(diag);
                    }
                }
            }
        }
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            if !ALLOWED_OUTPUT_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, out_offset);
                let mut diag = Diagnostic::error(
                    "E0076",
                    format!("Unknown child node '{}' in output configuration of tool '{}'", child_name, tool_name),
                )
                .with_help(format!("Allowed output options are: {}.", ALLOWED_OUTPUT_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_OUTPUT_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            } else {
                let child_offset = SourceLocation::find_node_name(input, child_name, out_offset).map(|l| l.offset);
                match child_name {
                    "slice" => {
                        for entry in child.entries() {
                            if let Some(prop_name) = entry.name() {
                                let key = prop_name.value();
                                if !ALLOWED_SLICE_PROPS.contains(&key) {
                                    let loc = SourceLocation::find_property(input, key, child_offset);
                                    let mut diag = Diagnostic::error(
                                        "E0076",
                                        format!("Unknown property '{}' on slice node in tool '{}'", key, tool_name),
                                    )
                                    .with_help(format!("Allowed slice properties are: {}.", ALLOWED_SLICE_PROPS.join(", ")));
                                    if let Some(closest) = find_closest_match(key, ALLOWED_SLICE_PROPS.iter().copied()) {
                                        diag = diag.with_note(format!("Did you mean '{}'?", closest));
                                    }
                                    if let Some(l) = loc {
                                        diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                                    }
                                    report.add(diag);
                                }
                            }
                        }
                        let lines_val = get_prop_u64(child, "lines").or_else(|| get_arg_u64(child, 0));
                        if let Some(0) = lines_val {
                            let loc = SourceLocation::find_in_source(input, "0", child_offset);
                            let mut diag = Diagnostic::error(
                                "E0077",
                                format!("Slice line count must be greater than 0 in tool '{}'", tool_name),
                            )
                            .with_help("Set slice line count to a positive integer (e.g. lines=25).");
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label("lines must be > 0");
                            }
                            report.add(diag);
                        }
                    }
                    "regex" => {
                        for entry in child.entries() {
                            if let Some(prop_name) = entry.name() {
                                let key = prop_name.value();
                                if !ALLOWED_REGEX_PROPS.contains(&key) {
                                    let loc = SourceLocation::find_property(input, key, child_offset);
                                    let mut diag = Diagnostic::error(
                                        "E0076",
                                        format!("Unknown property '{}' on regex node in tool '{}'", key, tool_name),
                                    )
                                    .with_help(format!("Allowed regex properties are: {}.", ALLOWED_REGEX_PROPS.join(", ")));
                                    if let Some(closest) = find_closest_match(key, ALLOWED_REGEX_PROPS.iter().copied()) {
                                        diag = diag.with_note(format!("Did you mean '{}'?", closest));
                                    }
                                    if let Some(l) = loc {
                                        diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                                    }
                                    report.add(diag);
                                }
                            }
                        }
                        if let Some(regex_children) = child.children() {
                            for rc in regex_children.nodes() {
                                let rc_name = rc.name().value();
                                if !ALLOWED_REGEX_CHILDREN.contains(&rc_name) {
                                    let loc = SourceLocation::find_node_name(input, rc_name, child_offset);
                                    let mut diag = Diagnostic::error(
                                        "E0076",
                                        format!("Unknown child node '{}' in regex block of tool '{}'", rc_name, tool_name),
                                    )
                                    .with_help(format!("Allowed regex options are: {}.", ALLOWED_REGEX_CHILDREN.join(", ")));
                                    if let Some(closest) = find_closest_match(rc_name, ALLOWED_REGEX_CHILDREN.iter().copied()) {
                                        diag = diag.with_note(format!("Did you mean '{}'?", closest));
                                    }
                                    if let Some(l) = loc {
                                        diag = diag.with_location(l).with_label(format!("unknown child node '{}'", rc_name));
                                    }
                                    report.add(diag);
                                }
                            }
                        }
                        let pat_opt = get_arg_str(child, 0).or_else(|| get_prop(child, "pattern"));
                        if let Some(ref pat) = pat_opt {
                            if let Err(e) = validate_regex_pattern(pat) {
                                let loc = SourceLocation::find_in_source(input, pat, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0078",
                                    format!("Invalid regex pattern '{}' in output configuration of tool '{}': {}", pat, tool_name, e),
                                )
                                .with_help("Ensure the regex pattern is a valid regular expression.");
                                if let Some(l) = loc {
                                    diag = diag.with_location(l).with_label("invalid regex syntax");
                                }
                                report.add(diag);
                            }
                        } else {
                            let loc = SourceLocation::find_node_name(input, "regex", child_offset);
                            let mut diag = Diagnostic::error(
                                "E0078",
                                format!("Regex transformer on tool '{}' is missing a pattern", tool_name),
                            )
                            .with_help("Specify a pattern: regex \"^pattern\" { template \"...\" }");
                            if let Some(l) = loc {
                                diag = diag.with_location(l).with_label("missing pattern");
                            }
                            report.add(diag);
                        }
                    }
                    "filter-not" | "filter_not" | "filter" => {
                        let pat_opt = get_arg_str(child, 0).or_else(|| get_prop(child, "pattern"));
                        if let Some(ref pat) = pat_opt {
                            if let Err(e) = validate_regex_pattern(pat) {
                                let loc = SourceLocation::find_in_source(input, pat, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0078",
                                    format!("Invalid filter pattern '{}' in output configuration of tool '{}': {}", pat, tool_name, e),
                                )
                                .with_help("Ensure the filter pattern is a valid regular expression.");
                                if let Some(l) = loc {
                                    diag = diag.with_location(l).with_label("invalid regex syntax");
                                }
                                report.add(diag);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let raw_format = get_prop(node, "format").or_else(|| get_arg_str(node, 0));
    if let Some(fmt) = raw_format {
        if !ALLOWED_OUTPUT_FORMATS.contains(&fmt.to_lowercase().as_str()) {
            let loc = SourceLocation::find_in_source(input, &fmt, out_offset);
            let mut diag = Diagnostic::error(
                "E0075",
                format!("Unsupported output format '{}' on tool '{}'", fmt, tool_name),
            )
            .with_help("Supported output formats are: text, image, json.");
            if let Some(closest) = find_closest_match(&fmt.to_lowercase(), ALLOWED_OUTPUT_FORMATS.iter().copied()) {
                diag = diag.with_note(format!("Did you mean '{}'?", closest));
            }
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("unsupported format '{}'", fmt));
            }
            report.add(diag);
        }
    }
}

pub(crate) fn parse_output_node(node: &KdlNode) -> Result<OutputSpec, ParserError> {
    let mut raw_format = get_prop(node, "format").or_else(|| get_arg_str(node, 0));
    let mut mime_type = get_prop(node, "mime-type").or_else(|| get_prop(node, "mime_type"));
    let mut trim = get_prop_bool(node, "trim").unwrap_or(false);
    let mut slice = None;
    let mut extract_json = get_prop(node, "extract-json").or_else(|| get_prop(node, "extract_json"));
    let mut filter_not = get_prop(node, "filter-not").or_else(|| get_prop(node, "filter_not"));
    let mut filter = get_prop(node, "filter");
    let mut regex = None;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            match child_name {
                "format" => {
                    if let Some(fmt) = get_arg_str(child, 0)
                        .or_else(|| get_prop(child, "name"))
                        .or_else(|| get_prop(child, "value"))
                    {
                        raw_format = Some(fmt);
                    }
                }
                "mime-type" | "mime_type" => {
                    if let Some(mt) = get_arg_str(child, 0).or_else(|| get_prop(child, "value")) {
                        mime_type = Some(mt);
                    }
                }
                "trim" => {
                    trim = get_arg_bool(child, 0)
                        .or_else(|| get_prop_bool(child, "enabled"))
                        .or_else(|| get_prop_bool(child, "value"))
                        .unwrap_or(true);
                }
                "slice" => {
                    let lines = get_prop_u64(child, "lines")
                        .or_else(|| get_arg_u64(child, 0))
                        .unwrap_or(10) as usize;
                    let head = if let Some(h) = get_prop_bool(child, "head") {
                        h
                    } else if let Some(t) = get_prop_bool(child, "tail") {
                        !t
                    } else {
                        true
                    };
                    slice = Some(OutputSliceSpec { lines, head });
                }
                "extract-json" | "extract_json" => {
                    if let Some(path) = get_arg_str(child, 0)
                        .or_else(|| get_prop(child, "path"))
                        .or_else(|| get_prop(child, "pointer"))
                    {
                        extract_json = Some(path);
                    }
                }
                "filter-not" | "filter_not" => {
                    if let Some(pat) = get_arg_str(child, 0).or_else(|| get_prop(child, "pattern")) {
                        filter_not = Some(pat);
                    }
                }
                "filter" => {
                    if let Some(pat) = get_arg_str(child, 0).or_else(|| get_prop(child, "pattern")) {
                        filter = Some(pat);
                    }
                }
                "regex" => {
                    let pattern = get_arg_str(child, 0)
                        .or_else(|| get_prop(child, "pattern"))
                        .unwrap_or_default();
                    let mut template = get_prop(child, "template");
                    if let Some(r_children) = child.children() {
                        for rc in r_children.nodes() {
                            if rc.name().value() == "template" {
                                template = get_arg_str(rc, 0).or_else(|| get_prop(rc, "value"));
                            }
                        }
                    }
                    regex = Some(OutputRegexSpec { pattern, template });
                }
                _ => {}
            }
        }
    }

    let format = match raw_format.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("image" | "img" | "png" | "jpeg") => OutputFormat::Image,
        Some("json") => OutputFormat::Json,
        _ => OutputFormat::Text,
    };

    if mime_type.is_none() {
        mime_type = match format {
            OutputFormat::Text => Some("text/plain".to_string()),
            OutputFormat::Image => Some("image/png".to_string()),
            OutputFormat::Json => Some("application/json".to_string()),
        };
    }

    Ok(OutputSpec {
        format,
        mime_type,
        trim,
        slice,
        extract_json,
        regex,
        filter_not,
        filter,
    })
}
