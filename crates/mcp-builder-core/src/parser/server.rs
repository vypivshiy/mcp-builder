use crate::ast::{EnvType, EnvVarSpec, ServerSpec, TransportSpec};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::*;
use crate::parser::ParserError;
use kdl::KdlNode;

pub const ALLOWED_SERVER_PROPS: &[&str] = &[
    "name",
    "version",
    "description",
    "author",
    "license",
    "protocol-version",
    "protocol_version",
    "profile",
];

pub const ALLOWED_SERVER_CHILDREN: &[&str] = &[
    "description",
    "version",
    "author",
    "license",
    "protocol-version",
    "protocol_version",
    "profile",
];

pub const ALLOWED_TRANSPORTS: &[&str] = &["stdio", "sse"];
pub const ALLOWED_STDIO_PROPS: &[&str] = &["enabled"];

pub const ALLOWED_SSE_PROPS: &[&str] = &[
    "enabled",
    "host",
    "port",
    "endpoint",
    "message-endpoint",
    "message_endpoint",
    "cors",
];

pub const ALLOWED_SSE_CHILDREN: &[&str] = &[
    "host",
    "port",
    "endpoint",
    "message-endpoint",
    "message_endpoint",
    "cors",
];

pub const ALLOWED_ENV_PROPS: &[&str] = &[
    "type",
    "required",
    "default",
    "description",
    "cli",
    "short",
];

pub const ALLOWED_ENV_CHILDREN: &[&str] = &[
    "default",
    "description",
    "required",
    "cli",
    "short",
    "type",
];

pub const ALLOWED_ENV_TYPES: &[&str] = &[
    "string", "path", "dir", "file", "integer", "int", "i32", "i64", "u16", "port", "boolean", "bool",
];

pub(crate) fn validate_server_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let server_offset = SourceLocation::find_node_name(input, "server", None).map(|l| l.offset);

    let name_opt = get_prop(node, "name").or_else(|| get_arg_str(node, 0));
    if name_opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        let loc = SourceLocation::find_node_name(input, "server", server_offset);
        let mut diag = Diagnostic::error("E0004", "Server declaration is missing a 'name' attribute")
            .with_help("Specify a name for the server, e.g.: server name=\"my-server\" version=\"1.0.0\"");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("missing server name");
        }
        report.add(diag);
    }

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            if !ALLOWED_SERVER_PROPS.contains(&key) {
                let loc = SourceLocation::find_property(input, key, server_offset);
                let mut diag = Diagnostic::error("E0040", format!("Unknown property '{}' on 'server' node", key))
                    .with_help(format!("Allowed server properties are: {}.", ALLOWED_SERVER_PROPS.join(", ")));
                if let Some(closest) = find_closest_match(key, ALLOWED_SERVER_PROPS.iter().copied()) {
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
            if !ALLOWED_SERVER_CHILDREN.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, server_offset);
                let mut diag = Diagnostic::error("E0040", format!("Unknown child node '{}' in server block", child_name))
                    .with_help(format!("Allowed children in server block are: {}.", ALLOWED_SERVER_CHILDREN.join(", ")));
                if let Some(closest) = find_closest_match(child_name, ALLOWED_SERVER_CHILDREN.iter().copied()) {
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

pub(crate) fn validate_transports_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let transports_offset = SourceLocation::find_node_name(input, "transports", None).map(|l| l.offset);

    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            let loc = SourceLocation::find_property(input, key, transports_offset);
            let mut diag = Diagnostic::error("E0041", format!("Unknown property '{}' on transports block", key))
                .with_help("The 'transports' block does not accept top-level properties. Declare 'stdio' or 'sse' inside.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
            }
            report.add(diag);
        }
    }

    let mut stdio_enabled = true;
    let mut sse_enabled = false;
    let mut stdio_declared = false;
    let mut sse_declared = false;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let child_name = child.name().value();
            if !ALLOWED_TRANSPORTS.contains(&child_name) {
                let loc = SourceLocation::find_node_name(input, child_name, transports_offset);
                let mut diag = Diagnostic::error("E0041", format!("Unknown transport '{}' in transports block", child_name))
                    .with_help("Supported transports are 'stdio' and 'sse'.");
                if let Some(closest) = find_closest_match(child_name, ALLOWED_TRANSPORTS.iter().copied()) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown transport '{}'", child_name));
                }
                report.add(diag);
                continue;
            }

            let child_offset = SourceLocation::find_node_name(input, child_name, transports_offset).map(|l| l.offset);

            match child_name {
                "stdio" => {
                    stdio_declared = true;
                    stdio_enabled = get_prop_bool(child, "enabled")
                        .or_else(|| get_arg_bool(child, 0))
                        .unwrap_or(true);

                    for entry in child.entries() {
                        if let Some(prop_name) = entry.name() {
                            let key = prop_name.value();
                            if !ALLOWED_STDIO_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error("E0041", format!("Unknown property '{}' on 'stdio' transport", key))
                                    .with_help("Allowed property for 'stdio' is 'enabled'.");
                                if let Some(l) = loc {
                                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                "sse" => {
                    sse_declared = true;
                    sse_enabled = get_prop_bool(child, "enabled")
                        .or_else(|| get_arg_bool(child, 0))
                        .unwrap_or(true);

                    for entry in child.entries() {
                        if let Some(prop_name) = entry.name() {
                            let key = prop_name.value();
                            if !ALLOWED_SSE_PROPS.contains(&key) {
                                let loc = SourceLocation::find_property(input, key, child_offset);
                                let mut diag = Diagnostic::error("E0041", format!("Unknown property '{}' on 'sse' transport", key))
                                    .with_help(format!("Allowed 'sse' properties are: {}.", ALLOWED_SSE_PROPS.join(", ")));
                                if let Some(closest) = find_closest_match(key, ALLOWED_SSE_PROPS.iter().copied()) {
                                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                                }
                                if let Some(l) = loc {
                                    diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                                }
                                report.add(diag);
                            }
                        }
                    }

                    if let Some(sse_children) = child.children() {
                        for sc in sse_children.nodes() {
                            let sc_name = sc.name().value();
                            if !ALLOWED_SSE_CHILDREN.contains(&sc_name) {
                                let loc = SourceLocation::find_node_name(input, sc_name, child_offset);
                                let mut diag = Diagnostic::error("E0041", format!("Unknown child node '{}' in 'sse' transport block", sc_name))
                                    .with_help(format!("Allowed 'sse' configuration options are: {}.", ALLOWED_SSE_CHILDREN.join(", ")));
                                if let Some(closest) = find_closest_match(sc_name, ALLOWED_SSE_CHILDREN.iter().copied()) {
                                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                                }
                                if let Some(l) = loc {
                                    diag = diag.with_location(l).with_label(format!("unknown child node '{}'", sc_name));
                                }
                                report.add(diag);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if (stdio_declared || sse_declared) && !stdio_enabled && !sse_enabled {
        let loc = SourceLocation::find_node_name(input, "transports", transports_offset);
        let mut diag = Diagnostic::error(
            "E0042",
            "All transports are disabled. At least one transport ('stdio' or 'sse') must be enabled.",
        )
        .with_help("Enable at least one transport, e.g.: stdio enabled=#true");
        if let Some(l) = loc {
            diag = diag.with_location(l).with_label("all transports disabled");
        }
        report.add(diag);
    }
}

pub(crate) fn validate_env_node(node: &KdlNode, input: &str, report: &mut DiagnosticReport) {
    let env_offset = SourceLocation::find_node_name(input, "env", None).map(|l| l.offset);

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let var_name = child.name().value();
            let var_offset = SourceLocation::find_node_name(input, var_name, env_offset).map(|l| l.offset);

            for entry in child.entries() {
                if let Some(prop_name) = entry.name() {
                    let key = prop_name.value();
                    if !ALLOWED_ENV_PROPS.contains(&key) {
                        let loc = SourceLocation::find_property(input, key, var_offset);
                        let mut diag = Diagnostic::error("E0043", format!("Unknown property '{}' on environment variable '{}'", key, var_name))
                            .with_help(format!("Allowed env properties are: {}.", ALLOWED_ENV_PROPS.join(", ")));
                        if let Some(closest) = find_closest_match(key, ALLOWED_ENV_PROPS.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{}'?", closest));
                        }
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("unknown property '{}'", key));
                        }
                        report.add(diag);
                    }
                }
            }

            if let Some(c_children) = child.children() {
                for cc in c_children.nodes() {
                    let cc_name = cc.name().value();
                    if !ALLOWED_ENV_CHILDREN.contains(&cc_name) {
                        let loc = SourceLocation::find_node_name(input, cc_name, var_offset);
                        let mut diag = Diagnostic::error("E0043", format!("Unknown child node '{}' on environment variable '{}'", cc_name, var_name))
                            .with_help(format!("Allowed env children are: {}.", ALLOWED_ENV_CHILDREN.join(", ")));
                        if let Some(closest) = find_closest_match(cc_name, ALLOWED_ENV_CHILDREN.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{}'?", closest));
                        }
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("unknown child node '{}'", cc_name));
                        }
                        report.add(diag);
                    }
                }
            }

            let mut raw_type = get_prop(child, "type");
            if raw_type.is_none() {
                if let Some(entry) = child.entries().first() {
                    if let Some(ty) = entry.ty() {
                        raw_type = Some(ty.value().to_string());
                    }
                }
            }
            if raw_type.is_none() {
                if let Some(ty) = child.ty() {
                    raw_type = Some(ty.value().to_string());
                }
            }

            if let Some(ty_str) = raw_type {
                if !ALLOWED_ENV_TYPES.contains(&ty_str.to_lowercase().as_str()) {
                    let loc = SourceLocation::find_in_source(input, &ty_str, var_offset);
                    let mut diag = Diagnostic::error("E0044", format!("Unknown environment variable type '{}' on '{}'", ty_str, var_name))
                        .with_help("Supported environment variable types are: string, path, integer, u16, boolean.");
                    if let Some(closest) = find_closest_match(&ty_str.to_lowercase(), ALLOWED_ENV_TYPES.iter().copied()) {
                        diag = diag.with_note(format!("Did you mean '{}'?", closest));
                    }
                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label(format!("unknown env type '{}'", ty_str));
                    }
                    report.add(diag);
                }
            }
        }
    }
}

pub(crate) fn parse_server_node(node: &KdlNode, server: &mut ServerSpec) -> Result<(), ParserError> {
    if let Some(name_val) = get_prop(node, "name").or_else(|| get_arg_str(node, 0)) {
        server.name = name_val;
    }
    if let Some(ver_val) = get_prop(node, "version") {
        server.version = ver_val;
    }
    if let Some(desc_val) = get_prop(node, "description") {
        server.description = Some(desc_val);
    }
    if let Some(author_val) = get_prop(node, "author") {
        server.author = Some(author_val);
    }
    if let Some(license_val) = get_prop(node, "license") {
        server.license = Some(license_val);
    }
    if let Some(proto_val) = get_prop(node, "protocol-version").or_else(|| get_prop(node, "protocol_version")) {
        server.protocol_version = proto_val;
    }
    if let Some(prof_val) = get_prop(node, "profile").or_else(|| get_arg_str(node, 1)) {
        server.default_profile = Some(prof_val);
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "description" => {
                    server.description = get_arg_str(child, 0);
                }
                "version" => {
                    if let Some(v) = get_arg_str(child, 0) {
                        server.version = v;
                    }
                }
                "author" => {
                    server.author = get_arg_str(child, 0);
                }
                "license" => {
                    server.license = get_arg_str(child, 0);
                }
                "protocol-version" | "protocol_version" => {
                    if let Some(p) = get_arg_str(child, 0) {
                        server.protocol_version = p;
                    }
                }
                "profile" => {
                    if let Some(prof) = get_arg_str(child, 0).or_else(|| get_prop(child, "name")) {
                        server.default_profile = Some(prof);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_transports_node(node: &KdlNode, transports: &mut TransportSpec) -> Result<(), ParserError> {
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "stdio" => {
                    transports.stdio = get_prop_bool(child, "enabled")
                        .or_else(|| get_arg_bool(child, 0))
                        .unwrap_or(true);
                }
                "sse" => {
                    transports.sse = get_prop_bool(child, "enabled")
                        .or_else(|| get_arg_bool(child, 0))
                        .unwrap_or(true);
                    if let Some(sse_children) = child.children() {
                        for sc in sse_children.nodes() {
                            match sc.name().value() {
                                "host" => transports.sse_host = get_arg_str(sc, 0),
                                "port" => transports.sse_port = get_arg_u64(sc, 0).map(|p| p as u16),
                                "endpoint" => transports.sse_endpoint = get_arg_str(sc, 0),
                                "message-endpoint" | "message_endpoint" => transports.sse_message_endpoint = get_arg_str(sc, 0),
                                "cors" => transports.cors = get_arg_bool(sc, 0).unwrap_or(true),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_env_node(node: &KdlNode, envs: &mut Vec<EnvVarSpec>) -> Result<(), ParserError> {
    if let Some(children) = node.children() {
        for child in children.nodes() {
            let name = child.name().value().to_string();
            let mut raw_type = get_prop(child, "type");
            if raw_type.is_none() {
                if let Some(entry) = child.entries().first() {
                    if let Some(ty) = entry.ty() {
                        raw_type = Some(ty.value().to_string());
                    }
                }
            }
            if raw_type.is_none() {
                if let Some(ty) = child.ty() {
                    raw_type = Some(ty.value().to_string());
                }
            }

            let var_type = match raw_type.as_deref().unwrap_or("string").to_lowercase().as_str() {
                "path" | "dir" | "file" => EnvType::Path,
                "integer" | "int" | "i32" | "i64" => EnvType::Integer,
                "u16" | "port" => EnvType::U16,
                "boolean" | "bool" => EnvType::Boolean,
                _ => EnvType::String,
            };

            let mut required = get_prop_bool(child, "required").unwrap_or(false);
            let mut default = get_prop(child, "default").or_else(|| get_arg_str(child, 0));
            let mut description = get_prop(child, "description");
            let mut cli = get_prop(child, "cli");
            let mut short = get_prop(child, "short");

            if let Some(c_children) = child.children() {
                for cc in c_children.nodes() {
                    match cc.name().value() {
                        "default" => default = get_arg_str(cc, 0),
                        "description" => description = get_arg_str(cc, 0),
                        "required" => required = get_arg_bool(cc, 0).unwrap_or(true),
                        "cli" => cli = get_arg_str(cc, 0),
                        "short" => short = get_arg_str(cc, 0),
                        _ => {}
                    }
                }
            }

            envs.push(EnvVarSpec {
                name,
                var_type,
                required,
                default,
                description,
                cli,
                short,
            });
        }
    }
    Ok(())
}
