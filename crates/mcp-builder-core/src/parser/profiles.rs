use crate::ast::{ExecBinding, ProfileSpec, TargetOs, ToolBinding, ToolProfileRef};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::bindings::*;
use crate::parser::output::{parse_output_node, validate_output_node};
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_PROFILE_PROPS: &[&str] = &[
    "name",
    "extends",
    "description",
    "method",
    "url",
    "host",
    "port",
    "command",
    "progid",
    "framing",
    "message",
    "extract-json",
    "extract_json",
    "timeout-ms",
    "timeout_ms",
];

pub const ALLOWED_PROFILE_CHILDREN: &[&str] = &[
    "description",
    "extends",
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
    "output",
    "method",
    "url",
    "host",
    "port",
    "command",
    "args",
    "arg",
    "progid",
    "framing",
    "message",
    "headers",
    "header",
    "params",
    "extract-json",
    "extract_json",
    "timeout-ms",
    "timeout_ms",
];

pub const ALLOWED_TOOL_PROFILE_PROPS: &[&str] = &[
    "name",
    "method",
    "url",
    "command",
    "host",
    "port",
    "progid",
    "framing",
    "message",
    "extract-json",
    "extract_json",
    "timeout-ms",
    "timeout_ms",
];

pub const ALLOWED_TOOL_PROFILE_CHILDREN: &[&str] = &[
    "method",
    "url",
    "command",
    "args",
    "arg",
    "host",
    "port",
    "progid",
    "framing",
    "message",
    "headers",
    "header",
    "params",
    "extract-json",
    "extract_json",
    "timeout-ms",
    "timeout_ms",
];

pub(crate) fn validate_profile_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let name_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "name"));
    let prof_name = name_opt.clone().unwrap_or_else(|| "unnamed_profile".to_string());
    let prof_offset = SourceLocation::find_in_source(input, &format!("\"{}\"", prof_name), None)
        .or_else(|| SourceLocation::find_node_name(input, "profile", None))
        .map(|l| l.offset);

    if name_opt.is_none() || prof_name.trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, "profile", prof_offset);
        let mut diag = Diagnostic::error("E0067", "Profile declaration is missing a name")
            .with_help("Provide a profile name, e.g.: profile \"my_profile\" { ... }");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing profile name");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_PROFILE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, prof_offset);
                let mut diag = Diagnostic::error(
                    "E0067",
                    format!("Unknown property '{}' on profile '{}'", key, prof_name),
                )
                .with_help(format!("Allowed profile properties are: {}.", ALLOWED_PROFILE_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_PROFILE_PROPS.iter().copied()) {
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
            let child_offset = SourceLocation::find_node_name(input, child_name, prof_offset).map(|l| l.offset);

            if !ALLOWED_PROFILE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, prof_offset);
                let mut diag = Diagnostic::error(
                    "E0067",
                    format!("Unknown child node '{}' in profile '{}'", child_name, prof_name),
                )
                .with_help(format!("Allowed children in a 'profile' block are: {}.", ALLOWED_PROFILE_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_PROFILE_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
                continue;
            }

            match child_name {
                "exec" | "bind:exec" | "exec:windows" | "bind:exec:windows" | "exec:linux" | "bind:exec:linux"
                | "exec:macos" | "bind:exec:macos" | "exec:darwin" | "bind:exec:darwin" | "exec:osx" | "bind:exec:osx"
                | "exec:unix" | "bind:exec:unix" | "exec:fallback" | "bind:exec:fallback" | "exec:default" | "bind:exec:default"
                | "fallback" | "default" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_EXEC_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0056",
                                    format!("Unknown property '{}' in exec binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_EXEC_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0056",
                                    format!("Unknown child node '{}' in exec binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "http" | "bind:http" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_HTTP_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0059",
                                    format!("Unknown property '{}' in http binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_HTTP_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0059",
                                    format!("Unknown child node '{}' in http binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "ipc" | "bind:ipc" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_IPC_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0060",
                                    format!("Unknown property '{}' in ipc binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_IPC_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0060",
                                    format!("Unknown child node '{}' in ipc binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "native" | "bind:native" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_NATIVE_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0061",
                                    format!("Unknown property '{}' in native binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "com" | "bind:com" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_COM_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0062",
                                    format!("Unknown property '{}' in com binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_COM_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0062",
                                    format!("Unknown child node '{}' in com binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "ws" | "bind:ws" | "websocket" | "bind:websocket" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_WS_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0063",
                                    format!("Unknown property '{}' in ws binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_WS_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0063",
                                    format!("Unknown child node '{}' in ws binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "pipe" | "bind:pipe" | "tcp" | "bind:tcp" => {
                    for entry in child.entries() {
                        if let Some(p) = entry.name() {
                            let key = p.value();
                            if !ALLOWED_PIPE_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0064",
                                    format!("Unknown property '{}' in pipe binding on profile '{}'", key, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                    if let Some(c_nodes) = child.children() {
                        for cc in c_nodes.nodes() {
                            let cc_name = cc.name().value();
                            if !ALLOWED_PIPE_CHILDREN.contains(&cc_name) {
                                let loc = SourceLocation::find_node_name(input, cc_name, child_offset);
                                let mut diag = Diagnostic::error(
                                    "E0064",
                                    format!("Unknown child node '{}' in pipe binding on profile '{}'", cc_name, prof_name),
                                );
                                if let Some(l) = loc {
                                    diag = diag.with_location(l);
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "output" => {
                    validate_output_node(child, input, &prof_name, child_offset, report);
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn validate_tool_profile_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    parent_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let name_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "name"));
    let prof_offset = SourceLocation::find_node_name(input, node.name().value(), parent_offset).map(|l| l.offset);

    if name_opt.is_none() || name_opt.as_deref().unwrap_or("").trim().is_empty() {
        let loc = SourceLocation::find_node_name(input, node.name().value(), prof_offset);
        let mut diag = Diagnostic::error(
            "E0067",
            format!("Profile declaration on tool '{}' is missing a profile name", tool_name),
        )
        .with_help("Provide the profile name, e.g.: profile \"rest-json\" or profile \"cdp-chrome\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing profile name");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_TOOL_PROFILE_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, prof_offset);
                let mut diag = Diagnostic::error(
                    "E0067",
                    format!("Unknown property '{}' in profile block on tool '{}'", key, tool_name),
                )
                .with_help(format!("Allowed profile properties on tool are: {}.", ALLOWED_TOOL_PROFILE_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_TOOL_PROFILE_PROPS.iter().copied()) {
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
            if !ALLOWED_TOOL_PROFILE_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, prof_offset);
                let mut diag = Diagnostic::error(
                    "E0067",
                    format!("Unknown child node '{}' in profile block on tool '{}'", child_name, tool_name),
                )
                .with_help(format!("Allowed children in tool profile block are: {}.", ALLOWED_TOOL_PROFILE_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_TOOL_PROFILE_CHILDREN.iter().copied()) {
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

pub(crate) fn parse_profile_node(node: &KdlNode) -> Result<ProfileSpec, ParserError> {
    let name = get_arg_str(node, 0).or_else(|| get_prop(node, "name")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "profile".to_string(),
            message: "Profile declaration is missing a name".to_string(),
        }
    })?;

    let mut extends = get_prop(node, "extends");
    let mut description = get_prop(node, "description");
    let mut binding = None;
    let mut output = None;
    let mut method = get_prop(node, "method").or_else(|| get_prop(node, "dispatch")).or_else(|| get_prop(node, "call"));
    let mut url = get_prop(node, "url");
    let mut host = get_prop(node, "host");
    let mut port = get_prop(node, "port");
    let mut command = get_prop(node, "command");
    let mut args = Vec::new();
    let mut progid = get_prop(node, "progid");
    let mut framing = get_prop(node, "framing").or_else(|| get_prop(node, "delimiter"));
    let mut message = get_prop(node, "message").or_else(|| get_prop(node, "payload")).or_else(|| get_prop(node, "body")).or_else(|| get_prop(node, "send"));
    let mut headers = Vec::new();
    let mut params = Vec::new();
    let mut extract_json = get_prop(node, "extract-json").or_else(|| get_prop(node, "extract_json")).or_else(|| get_prop(node, "extract"));
    let mut timeout_ms = get_prop_u64(node, "timeout-ms").or_else(|| get_prop_u64(node, "timeout_ms")).or_else(|| get_prop_u64(node, "timeout"));

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "description" => {
                    description = get_arg_str(child, 0);
                }
                "extends" => {
                    extends = get_arg_str(child, 0);
                }
                "output" => {
                    output = Some(parse_output_node(child)?);
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
                    binding = Some(ToolBinding::Http(parse_http_binding(child)?));
                }
                "ipc" | "bind:ipc" => {
                    binding = Some(ToolBinding::Ipc(parse_ipc_binding(child)?));
                }
                "native" | "bind:native" => {
                    binding = Some(ToolBinding::Native(parse_native_binding(child, &name)?));
                }
                "com" | "bind:com" => {
                    binding = Some(ToolBinding::Com(parse_com_binding(child)?));
                }
                "ws" | "bind:ws" | "websocket" | "bind:websocket" => {
                    binding = Some(ToolBinding::Ws(parse_ws_binding(child)?));
                }
                "pipe" | "bind:pipe" | "tcp" | "bind:tcp" => {
                    binding = Some(ToolBinding::Pipe(parse_pipe_binding(child)?));
                }
                "method" | "dispatch" | "call" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        method = Some(m);
                    }
                }
                "url" => {
                    if let Some(u) = get_arg_str(child, 0) {
                        url = Some(u);
                    }
                }
                "host" => {
                    if let Some(h) = get_arg_str(child, 0) {
                        host = Some(h);
                    }
                }
                "port" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        port = Some(p);
                    }
                }
                "command" => {
                    if let Some(c) = get_arg_str(child, 0) {
                        command = Some(c);
                    }
                }
                "args" => {
                    for entry in child.entries() {
                        if let Some(val) = entry.value().as_string() {
                            args.push(val.to_string());
                        }
                    }
                }
                "arg" => {
                    if let Some(a) = get_arg_str(child, 0) {
                        args.push(a);
                    }
                }
                "progid" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        progid = Some(p);
                    }
                }
                "framing" | "delimiter" => {
                    if let Some(f) = get_arg_str(child, 0) {
                        framing = Some(f);
                    }
                }
                "message" | "payload" | "body" | "send" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        message = Some(m);
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
                    if let Some(key) = get_prop(child, "name").or_else(|| get_arg_str(child, 0)) {
                        let val = get_prop(child, "value").or_else(|| get_arg_str(child, 1)).unwrap_or_default();
                        headers.push((key, val));
                    }
                }
                "params" => {
                    if let Some(p_children) = child.children() {
                        for pc in p_children.nodes() {
                            let p_name = pc.name().value().to_string();
                            let p_val = get_arg_str(pc, 0).unwrap_or_default();
                            params.push((p_name, p_val));
                        }
                    }
                }
                "extract-json" | "extract_json" | "extract" => {
                    if let Some(ej) = get_arg_str(child, 0) {
                        extract_json = Some(ej);
                    }
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(ms) = get_arg_u64(child, 0) {
                        timeout_ms = Some(ms);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(ProfileSpec {
        name,
        description,
        extends,
        binding,
        output,
        method,
        url,
        host,
        port,
        command,
        args,
        progid,
        framing,
        message,
        headers,
        params,
        extract_json,
        timeout_ms,
    })
}

pub(crate) fn parse_tool_profile_node(node: &KdlNode) -> Result<ToolProfileRef, ParserError> {
    let name = get_arg_str(node, 0).or_else(|| get_prop(node, "name")).ok_or_else(|| {
        ParserError::ValidationError {
            location: "profile".to_string(),
            message: "Profile reference missing profile name".to_string(),
        }
    })?;

    let mut method = get_prop(node, "method").or_else(|| get_prop(node, "dispatch")).or_else(|| get_prop(node, "call"));
    let mut url = get_prop(node, "url");
    let mut host = get_prop(node, "host");
    let mut port = get_prop(node, "port");
    let mut command = get_prop(node, "command");
    let mut args = Vec::new();
    let mut progid = get_prop(node, "progid");
    let mut framing = get_prop(node, "framing").or_else(|| get_prop(node, "delimiter"));
    let mut message = get_prop(node, "message").or_else(|| get_prop(node, "payload")).or_else(|| get_prop(node, "body")).or_else(|| get_prop(node, "send"));
    let mut headers = Vec::new();
    let mut params = Vec::new();
    let mut extract_json = get_prop(node, "extract-json").or_else(|| get_prop(node, "extract_json")).or_else(|| get_prop(node, "extract"));
    let mut timeout_ms = get_prop_u64(node, "timeout-ms").or_else(|| get_prop_u64(node, "timeout_ms")).or_else(|| get_prop_u64(node, "timeout"));

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "method" | "dispatch" | "call" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        method = Some(m);
                    }
                }
                "url" => {
                    if let Some(u) = get_arg_str(child, 0) {
                        url = Some(u);
                    }
                }
                "host" => {
                    if let Some(h) = get_arg_str(child, 0) {
                        host = Some(h);
                    }
                }
                "port" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        port = Some(p);
                    }
                }
                "command" => {
                    if let Some(c) = get_arg_str(child, 0) {
                        command = Some(c);
                    }
                }
                "args" => {
                    for entry in child.entries() {
                        if let Some(val) = entry.value().as_string() {
                            args.push(val.to_string());
                        }
                    }
                }
                "arg" => {
                    if let Some(a) = get_arg_str(child, 0) {
                        args.push(a);
                    }
                }
                "progid" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        progid = Some(p);
                    }
                }
                "framing" | "delimiter" => {
                    if let Some(f) = get_arg_str(child, 0) {
                        framing = Some(f);
                    }
                }
                "message" | "payload" | "body" | "send" => {
                    if let Some(m) = get_arg_str(child, 0) {
                        message = Some(m);
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
                    if let Some(key) = get_prop(child, "name").or_else(|| get_arg_str(child, 0)) {
                        let val = get_prop(child, "value").or_else(|| get_arg_str(child, 1)).unwrap_or_default();
                        headers.push((key, val));
                    }
                }
                "params" => {
                    if let Some(p_children) = child.children() {
                        for pc in p_children.nodes() {
                            let p_name = pc.name().value().to_string();
                            let p_val = get_arg_str(pc, 0).unwrap_or_default();
                            params.push((p_name, p_val));
                        }
                    }
                }
                "extract-json" | "extract_json" | "extract" => {
                    if let Some(ej) = get_arg_str(child, 0) {
                        extract_json = Some(ej);
                    }
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(ms) = get_arg_u64(child, 0) {
                        timeout_ms = Some(ms);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(ToolProfileRef {
        name,
        method,
        url,
        command,
        args,
        host,
        port,
        message,
        framing,
        progid,
        params,
        headers,
        extract_json,
        timeout_ms,
    })
}
