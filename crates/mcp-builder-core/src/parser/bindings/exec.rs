use crate::ast::{ExecBinding, ExecVariant, TargetOs};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_EXEC_PROPS: &[&str] = &[
    "os",
    "target",
    "command",
    "workdir",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "capture-stdout",
    "capture_stdout",
    "capture-stderr",
    "capture_stderr",
];

pub const ALLOWED_EXEC_CHILDREN: &[&str] = &[
    "args",
    "arg",
    "workdir",
    "timeout-ms",
    "timeout_ms",
    "timeout",
    "capture-stdout",
    "capture_stdout",
    "capture-stderr",
    "capture_stderr",
    "env",
    "when:windows",
    "windows",
    "when:linux",
    "linux",
    "when:macos",
    "when:darwin",
    "when:osx",
    "macos",
    "darwin",
    "osx",
    "when:unix",
    "unix",
    "when:fallback",
    "when:default",
    "fallback",
    "default",
    "when",
];

pub fn validate_exec_binding_node(
    node: &KdlNode,
    input: &str,
    tool_name: &str,
    binding_offset: Option<usize>,
    report: &mut DiagnosticReport,
) {
    let node_name = node.name().value();
    let command_opt = get_arg_str(node, 0).or_else(|| get_prop(node, "command"));
    let has_direct_command = command_opt.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);

    let mut child_variant_count = 0;
    let mut has_windows_variant = false;
    let mut has_linux_variant = false;
    let mut has_macos_variant = false;
    let mut has_unix_variant = false;
    let mut has_fallback_variant = false;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            let is_variant_child = matches!(
                child_name,
                "when:windows" | "windows" | "when:linux" | "linux"
                | "when:macos" | "when:darwin" | "when:osx" | "macos" | "darwin" | "osx"
                | "when:unix" | "unix" | "when:fallback" | "when:default" | "fallback" | "default" | "when"
            );

            if is_variant_child {
                child_variant_count += 1;
                let v_cmd = get_arg_str(child, 0)
                    .or_else(|| get_arg_str(child, 1))
                    .or_else(|| get_prop(child, "command"));
                if v_cmd.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                    let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                    let mut diag = Diagnostic::error("E0055", format!("Conditional exec variant '{}' on tool '{}' is missing a command", child_name, tool_name))
                        .with_help(format!("Specify a command: {} \"my-command\"", child_name));
                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label("missing command");
                    }
                    report.add(diag);
                }

                match child_name {
                    "when:windows" | "windows" => has_windows_variant = true,
                    "when:linux" | "linux" => has_linux_variant = true,
                    "when:macos" | "when:darwin" | "when:osx" | "macos" | "darwin" | "osx" => has_macos_variant = true,
                    "when:unix" | "unix" => has_unix_variant = true,
                    "when:fallback" | "when:default" | "fallback" | "default" => has_fallback_variant = true,
                    "when" => {
                        if let Some(os_prop) = get_prop(child, "os").or_else(|| get_arg_str(child, 0)) {
                            match os_prop.to_lowercase().as_str() {
                                "windows" | "win" => has_windows_variant = true,
                                "linux" => has_linux_variant = true,
                                "macos" | "darwin" | "osx" => has_macos_variant = true,
                                "unix" | "posix" => has_unix_variant = true,
                                _ => has_fallback_variant = true,
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if !has_direct_command && child_variant_count == 0 {
        let loc = SourceLocation::find_node_name(input, node_name, binding_offset);
        let mut diag = Diagnostic::error("E0055", format!("Exec binding on tool '{}' is missing a command", tool_name))
            .with_help("Specify a command: bind:exec \"my-cli\" or exec command=\"my-cli\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing command");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_EXEC_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, binding_offset);
                let mut diag = Diagnostic::error("E0056", format!("Unknown property '{}' in exec binding on tool '{}'", key, tool_name))
                    .with_help(format!("Allowed exec properties are: {}.", ALLOWED_EXEC_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_EXEC_PROPS.iter().copied()) {
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
            if !ALLOWED_EXEC_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, binding_offset);
                let mut diag = Diagnostic::error("E0056", format!("Unknown child node '{}' in exec binding on tool '{}'", child_name, tool_name))
                    .with_help(format!("Allowed exec children are: {}.", ALLOWED_EXEC_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_EXEC_CHILDREN.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", child_name));
                }
                report.add(diag);
            }
        }
    }

    if child_variant_count > 0 && !has_direct_command && !has_fallback_variant {
        let covers_unix = has_unix_variant || (has_linux_variant && has_macos_variant);
        let covers_all = has_windows_variant && covers_unix;
        if !covers_all {
            let mut missing = Vec::new();
            if !has_windows_variant { missing.push("windows"); }
            if !has_linux_variant && !has_unix_variant { missing.push("linux"); }
            if !has_macos_variant && !has_unix_variant { missing.push("macos"); }

            let loc = SourceLocation::find_node_name(input, node_name, binding_offset);
            let mut diag = Diagnostic::warning(
                "W0007",
                format!("Tool '{}' specifies conditional OS execution but lacks fallback or coverage for: {}", tool_name, missing.join(", ")),
            )
            .with_help("Provide a fallback command (e.g. fallback \"...\" or exec \"...\") or specify variants for all target operating systems.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label("incomplete platform coverage without fallback");
            }
            report.add(diag);
        }
    }
}

pub fn parse_exec_variant(node: &KdlNode, default_os: Option<TargetOs>) -> Result<ExecVariant, ParserError> {
    let os_prop = get_prop(node, "os").or_else(|| get_prop(node, "target"));
    let os = if let Some(p) = os_prop {
        TargetOs::from_str_loose(&p).unwrap_or(TargetOs::Fallback)
    } else if let Some(os) = default_os {
        os
    } else if let Some(arg0) = get_arg_str(node, 0) {
        if let Some(matched) = TargetOs::from_str_loose(&arg0) {
            matched
        } else {
            TargetOs::Fallback
        }
    } else {
        TargetOs::Fallback
    };

    let command = if default_os.is_none() && get_arg_str(node, 0).and_then(|s| TargetOs::from_str_loose(&s)).is_some() {
        get_arg_str(node, 1).or_else(|| get_prop(node, "command")).unwrap_or_default()
    } else {
        get_arg_str(node, 0).or_else(|| get_prop(node, "command")).unwrap_or_default()
    };

    let mut args = Vec::new();
    let mut workdir = get_prop(node, "workdir");
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout_ms"))
        .or_else(|| get_prop_u64(node, "timeout"));
    let mut capture_stdout = get_prop_bool(node, "capture-stdout").or_else(|| get_prop_bool(node, "capture_stdout"));
    let mut capture_stderr = get_prop_bool(node, "capture-stderr").or_else(|| get_prop_bool(node, "capture_stderr"));
    let mut envs = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "args" => {
                    for entry in child.entries() {
                        if entry.name().is_none() {
                            if let Some(s) = val_as_string(entry.value()) {
                                args.push(s);
                            }
                        }
                    }
                }
                "arg" => {
                    if let Some(s) = get_arg_str(child, 0) {
                        args.push(s);
                    }
                }
                "workdir" => {
                    workdir = get_arg_str(child, 0);
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(t) = get_arg_u64(child, 0) {
                        timeout_ms = Some(t);
                    }
                }
                "capture-stdout" | "capture_stdout" => {
                    capture_stdout = get_arg_bool(child, 0);
                }
                "capture-stderr" | "capture_stderr" => {
                    capture_stderr = get_arg_bool(child, 0);
                }
                "env" => {
                    if let (Some(k), Some(v)) = (get_arg_str(child, 0), get_arg_str(child, 1)) {
                        envs.push((k, v));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(ExecVariant {
        os,
        command,
        args,
        workdir,
        timeout_ms,
        capture_stdout,
        capture_stderr,
        envs,
    })
}

pub fn parse_exec_binding(node: &KdlNode) -> Result<ExecBinding, ParserError> {
    let command = get_arg_str(node, 0)
        .or_else(|| get_prop(node, "command"))
        .unwrap_or_default();

    let mut args = Vec::new();
    let mut workdir = get_prop(node, "workdir");
    let mut timeout_ms = get_prop_u64(node, "timeout-ms")
        .or_else(|| get_prop_u64(node, "timeout"))
        .unwrap_or(10_000);
    let mut capture_stdout = get_prop_bool(node, "capture-stdout").unwrap_or(true);
    let mut capture_stderr = get_prop_bool(node, "capture-stderr").unwrap_or(true);
    let mut envs = Vec::new();
    let mut variants = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "args" => {
                    for entry in child.entries() {
                        if entry.name().is_none() {
                            if let Some(s) = val_as_string(entry.value()) {
                                args.push(s);
                            }
                        }
                    }
                }
                "arg" => {
                    if let Some(s) = get_arg_str(child, 0) {
                        args.push(s);
                    }
                }
                "workdir" => {
                    workdir = get_arg_str(child, 0);
                }
                "timeout-ms" | "timeout_ms" | "timeout" => {
                    if let Some(t) = get_arg_u64(child, 0) {
                        timeout_ms = t;
                    }
                }
                "capture-stdout" | "capture_stdout" => {
                    capture_stdout = get_arg_bool(child, 0).unwrap_or(true);
                }
                "capture-stderr" | "capture_stderr" => {
                    capture_stderr = get_arg_bool(child, 0).unwrap_or(true);
                }
                "env" => {
                    if let (Some(k), Some(v)) = (get_arg_str(child, 0), get_arg_str(child, 1)) {
                        envs.push((k, v));
                    }
                }
                "when:windows" | "windows" => {
                    variants.push(parse_exec_variant(child, Some(TargetOs::Windows))?);
                }
                "when:linux" | "linux" => {
                    variants.push(parse_exec_variant(child, Some(TargetOs::Linux))?);
                }
                "when:macos" | "when:darwin" | "when:osx" | "macos" | "darwin" | "osx" => {
                    variants.push(parse_exec_variant(child, Some(TargetOs::Macos))?);
                }
                "when:unix" | "unix" => {
                    variants.push(parse_exec_variant(child, Some(TargetOs::Unix))?);
                }
                "when:fallback" | "when:default" | "fallback" | "default" => {
                    variants.push(parse_exec_variant(child, Some(TargetOs::Fallback))?);
                }
                "when" => {
                    variants.push(parse_exec_variant(child, None)?);
                }
                _ => {}
            }
        }
    }

    Ok(ExecBinding {
        command,
        args,
        workdir,
        timeout_ms,
        capture_stdout,
        capture_stderr,
        envs,
        variants,
    })
}
