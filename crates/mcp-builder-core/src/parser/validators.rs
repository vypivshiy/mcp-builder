use crate::ast::{ServerSpec, ToolBinding};
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use crate::helpers::{
    get_arg_str, get_prop, is_valid_modifier, parse_placeholder_tokens,
    validate_regex_pattern,
};
use crate::parser::profiles::validate_profile_node;
use crate::parser::prompts_resources::{
    validate_prompt_node, validate_resource_node, validate_resource_template_node,
};
use crate::parser::server::{
    validate_env_node, validate_server_node, validate_transports_node,
};
use crate::parser::tools::validate_tool_node;
use kdl::KdlDocument;

pub const ALLOWED_TOP_LEVEL: &[&str] = &[
    "server",
    "transports",
    "env",
    "profile",
    "tool",
    "resource",
    "resource-template",
    "prompt",
];

pub(crate) fn validate_document(doc: &KdlDocument, input: &str, report: &mut DiagnosticReport) {
    let mut server_count = 0;
    let has_server_default_profile = doc.nodes().iter().any(|n| {
        n.name().value() == "server"
            && (get_prop(n, "profile").is_some()
                || get_arg_str(n, 1).is_some()
                || n.children()
                    .map(|c| c.nodes().iter().any(|cn| cn.name().value() == "profile"))
                    .unwrap_or(false))
    });

    for node in doc.nodes() {
        let name = node.name().value();
        if name == "server" {
            server_count += 1;
        }

        if !ALLOWED_TOP_LEVEL.contains(&name) {
            let loc = SourceLocation::find_node_name(input, name, None);
            let mut diag = Diagnostic::error("E0003", format!("Unknown top-level node '{}'", name))
                .with_help(format!(
                    "Available top-level nodes are: {}.",
                    ALLOWED_TOP_LEVEL.join(", ")
                ));
            if let Some(closest) = find_closest_match(name, ALLOWED_TOP_LEVEL.iter().copied()) {
                diag = diag.with_note(format!("Did you mean '{}'?", closest));
            }
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("unknown top-level node '{}'", name));
            }
            report.add(diag);
            continue;
        }

        match name {
            "server" => validate_server_node(node, input, report),
            "transports" => validate_transports_node(node, input, report),
            "env" => validate_env_node(node, input, report),
            "profile" => validate_profile_node(node, input, report),
            "tool" => validate_tool_node(node, input, report, has_server_default_profile),
            "resource" => validate_resource_node(node, input, report),
            "resource-template" => validate_resource_template_node(node, input, report),
            "prompt" => validate_prompt_node(node, input, report),
            _ => {}
        }
    }

    if server_count == 0 {
        let mut diag = Diagnostic::error("E0002", "Missing required 'server' declaration")
            .with_help("Add a top-level 'server name=\"...\" version=\"...\"' block to declare the server identity.");
        if let Some(loc) = SourceLocation::from_offset(input, 0, 0).into() {
            diag = diag.with_location(loc);
        }
        report.add(diag);
    } else if server_count > 1 {
        let diag = Diagnostic::error(
            "E0005",
            format!("Multiple 'server' declarations found ({}). Only one root server block is allowed.", server_count),
        )
        .with_help("Combine server properties into a single top-level 'server' block.");
        report.add(diag);
    }
}

pub(crate) fn run_linter(input: &str, server: &ServerSpec, report: &mut DiagnosticReport) {
    // E0010: Duplicate tool names
    let mut seen_tools = std::collections::HashSet::new();
    for tool in &server.tools {
        if !seen_tools.insert(&tool.name) {
            let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", tool.name), None);
            let mut diag = Diagnostic::error(
                "E0010",
                format!("Duplicate tool declaration '{}'", tool.name),
            )
            .with_help("Each tool declared in a server must have a unique identifier.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("duplicate tool '{}'", tool.name));
            }
            report.add(diag);
        }
    }

    // E0011: Duplicate prompt names
    let mut seen_prompts = std::collections::HashSet::new();
    for prompt in &server.prompts {
        if !seen_prompts.insert(&prompt.name) {
            let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", prompt.name), None);
            let mut diag = Diagnostic::error(
                "E0011",
                format!("Duplicate prompt declaration '{}'", prompt.name),
            )
            .with_help("Each prompt must have a unique identifier.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("duplicate prompt '{}'", prompt.name));
            }
            report.add(diag);
        }
    }

    // E0012: Duplicate resource URIs
    let mut seen_resources = std::collections::HashSet::new();
    for res in &server.resources {
        if !seen_resources.insert(&res.uri) {
            let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", res.uri), None);
            let mut diag = Diagnostic::error(
                "E0012",
                format!("Duplicate resource URI declaration '{}'", res.uri),
            )
            .with_help("Each static resource must have a unique URI.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label(format!("duplicate resource '{}'", res.uri));
            }
            report.add(diag);
        }
    }

    // Parameter constraints and duplicates
    for tool in &server.tools {
        let mut seen_params = std::collections::HashSet::new();
        for param in &tool.params {
            if !seen_params.insert(&param.name) {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", param.name), None);
                let mut diag = Diagnostic::error(
                    "E0013",
                    format!("Duplicate parameter '{}' declared on tool '{}'", param.name, tool.name),
                )
                .with_help(format!("Parameter names on tool '{}' must be unique.", tool.name));
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("duplicate parameter '{}'", param.name));
                }
                report.add(diag);
            }

            if let (Some(min), Some(max)) = (param.minimum, param.maximum) {
                if min > max {
                    let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", param.name), None);
                    let mut diag = Diagnostic::error(
                        "E0031",
                        format!(
                            "Invalid numeric constraint on parameter '{}' of tool '{}': minimum ({}) is greater than maximum ({})",
                            param.name, tool.name, min, max
                        ),
                    )
                    .with_help("The minimum value cannot be greater than the maximum value.");
                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label("invalid minimum > maximum range");
                    }
                    report.add(diag);
                }
            }

            if let Some(pat) = &param.pattern {
                if let Err(e) = validate_regex_pattern(pat) {
                    let loc = SourceLocation::find_in_source(input, pat, None);
                    let mut diag = Diagnostic::error(
                        "E0032",
                        format!(
                            "Invalid regex pattern '{}' on parameter '{}' of tool '{}': {}",
                            pat, param.name, tool.name, e
                        ),
                    )
                    .with_help("Ensure the pattern is a valid regular expression.");
                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label("invalid regex syntax");
                    }
                    report.add(diag);
                }
            }
        }
    }

    // Semantic linting: environment variable and parameter references
    let declared_envs: std::collections::HashSet<&str> =
        server.envs.iter().map(|e| e.name.as_str()).collect();
    let mut used_envs = std::collections::HashSet::new();

    for tool in &server.tools {
        let declared_params: std::collections::HashSet<&str> =
            tool.params.iter().map(|p| p.name.as_str()).collect();
        let mut used_params = std::collections::HashSet::new();

        let mut texts_to_check: Vec<&str> = Vec::new();
        match &tool.binding {
            Some(ToolBinding::Exec(e)) => {
                texts_to_check.push(&e.command);
                for arg in &e.args {
                    texts_to_check.push(arg);
                }
                if let Some(wd) = &e.workdir {
                    texts_to_check.push(wd);
                }
                for (k, v) in &e.envs {
                    texts_to_check.push(k);
                    texts_to_check.push(v);
                }
                for v in &e.variants {
                    texts_to_check.push(&v.command);
                    for arg in &v.args {
                        texts_to_check.push(arg);
                    }
                    if let Some(wd) = &v.workdir {
                        texts_to_check.push(wd);
                    }
                    for (k, val) in &v.envs {
                        texts_to_check.push(k);
                        texts_to_check.push(val);
                    }
                }
            }
            Some(ToolBinding::Http(h)) => {
                texts_to_check.push(&h.url);
                for (k, v) in &h.headers {
                    texts_to_check.push(k);
                    texts_to_check.push(v);
                }
                for (k, v) in &h.query {
                    texts_to_check.push(k);
                    texts_to_check.push(v);
                }
                if let Some(b) = &h.body {
                    texts_to_check.push(b);
                }
            }
            Some(ToolBinding::Ipc(ipc)) => {
                texts_to_check.push(&ipc.dir);
                texts_to_check.push(&ipc.method);
                if let Some(p) = &ipc.params {
                    texts_to_check.push(p);
                }
            }
            Some(ToolBinding::Com(c)) => {
                texts_to_check.push(&c.progid);
                texts_to_check.push(&c.method);
                for arg in &c.args {
                    texts_to_check.push(arg);
                }
            }
            Some(ToolBinding::Ws(ws)) => {
                if let Some(u) = &ws.url {
                    texts_to_check.push(u);
                }
                if let Some(h) = &ws.host {
                    texts_to_check.push(h);
                }
                if let Some(p) = &ws.port {
                    texts_to_check.push(p);
                }
                if let Some(ep) = &ws.endpoint {
                    texts_to_check.push(ep);
                }
                texts_to_check.push(&ws.message);
                for (k, v) in &ws.headers {
                    texts_to_check.push(k);
                    texts_to_check.push(v);
                }
            }
            Some(ToolBinding::Pipe(pipe)) => {
                if let Some(a) = &pipe.addr {
                    texts_to_check.push(a);
                }
                if let Some(h) = &pipe.host {
                    texts_to_check.push(h);
                }
                if let Some(p) = &pipe.port {
                    texts_to_check.push(p);
                }
                texts_to_check.push(&pipe.message);
                texts_to_check.push(&pipe.framing);
            }
            _ => {}
        }

        for text in texts_to_check {
            let tokens = parse_placeholder_tokens(text);
            for token in tokens {
                if let Some(m) = &token.modifier {
                    if !is_valid_modifier(m) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None);
                        let mut diag = Diagnostic::error(
                            "E0025",
                            format!(
                                "Unknown placeholder modifier '{}' in '{}'",
                                m, token.raw
                            ),
                        )
                        .with_help("Allowed modifiers are: :json (serialize valid JSON value), :url (percent-encode).");

                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("unknown modifier ':{}'", m));
                        }

                        if let Some(cand) = find_closest_match(m, ["json", "url"].iter().copied()) {
                            diag = diag.with_note(format!("Did you mean ':{}'?", cand));
                        }

                        report.add(diag);
                    }
                }

                if token.is_env {
                    used_envs.insert(token.name.clone());
                    if !declared_envs.contains(token.name.as_str()) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None)
                            .or_else(|| SourceLocation::find_in_source(input, &format!("{{env:{}}}", token.name), None));
                        let mut diag = Diagnostic::error(
                            "E0021",
                            format!(
                                "Tool '{}' references undeclared environment variable '{{env:{}}}'",
                                tool.name, token.name
                            ),
                        )
                        .with_help(format!(
                            "Declare '{0} default=\"...\"' in the top-level 'env {{ ... }}' block.",
                            token.name
                        ));

                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("undeclared variable '{{env:{}}}'", token.name));
                        }

                        if let Some(cand) = find_closest_match(&token.name, declared_envs.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{{env:{}}}'?", cand));
                        }

                        report.add(diag);
                    }
                } else {
                    used_params.insert(token.name.clone());
                    if !declared_params.contains(token.name.as_str()) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None)
                            .or_else(|| SourceLocation::find_in_source(input, &format!("{{{}}}", token.name), None));
                        let mut diag = Diagnostic::error(
                            "E0020",
                            format!(
                                "Tool '{}' binding references undeclared parameter '{{{}}}'",
                                tool.name, token.name
                            ),
                        )
                        .with_help(format!(
                            "Declare 'param \"{}\"' on tool '{}' or check for typos.",
                            token.name, tool.name
                        ));

                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("undeclared parameter '{{{}}}'", token.name));
                        }

                        if let Some(cand) = find_closest_match(&token.name, declared_params.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{{{}}}'?", cand));
                        }

                        report.add(diag);
                    }
                }
            }
        }

        // Warnings on tools and parameters
        if tool.description.is_none() {
            let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", tool.name), None);
            let mut diag = Diagnostic::warning(
                "W0003",
                format!("Tool '{}' is missing a description", tool.name),
            )
            .with_help("Adding descriptions on tools helps LLMs understand when and how to call them.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label("missing description");
            }
            report.add(diag);
        }

        for param in &tool.params {
            if tool.binding.is_some() && !used_params.contains(&param.name) {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", param.name), None);
                let mut diag = Diagnostic::warning(
                    "W0001",
                    format!(
                        "Unused parameter '{}' declared on tool '{}'",
                        param.name, tool.name
                    ),
                )
                .with_help(format!(
                    "Remove 'param \"{}\"' or interpolate it into the execution binding via '{{{}}}'.",
                    param.name, param.name
                ));
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label("parameter declared but never referenced in binding");
                }
                report.add(diag);
            }

            if param.description.is_none() {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", param.name), None);
                let mut diag = Diagnostic::warning(
                    "W0004",
                    format!(
                        "Parameter '{}' on tool '{}' is missing a description",
                        param.name, tool.name
                    ),
                )
                .with_help("Adding descriptions on parameters guides LLMs to provide correct inputs.");
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label("missing parameter description");
                }
                report.add(diag);
            }

            if param.required && param.default.is_some() {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", param.name), None);
                let mut diag = Diagnostic::warning(
                    "W0006",
                    format!(
                        "Parameter '{}' on tool '{}' is marked required=#true but also defines a default value",
                        param.name, tool.name
                    ),
                )
                .with_help("Required parameters must always be supplied by callers, so the default value is shadowed.");
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label("shadowed default value");
                }
                report.add(diag);
            }
        }
    }

    for prompt in &server.prompts {
        let declared_args: std::collections::HashSet<&str> =
            prompt.arguments.iter().map(|a| a.name.as_str()).collect();

        for msg in &prompt.messages {
            let tokens = parse_placeholder_tokens(&msg.content);
            for token in tokens {
                if let Some(m) = &token.modifier {
                    if !is_valid_modifier(m) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None);
                        let mut diag = Diagnostic::error(
                            "E0025",
                            format!(
                                "Unknown placeholder modifier '{}' in '{}'",
                                m, token.raw
                            ),
                        )
                        .with_help("Allowed modifiers are: :json (serialize valid JSON value), :url (percent-encode).");

                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("unknown modifier ':{}'", m));
                        }

                        if let Some(cand) = find_closest_match(m, ["json", "url"].iter().copied()) {
                            diag = diag.with_note(format!("Did you mean ':{}'?", cand));
                        }

                        report.add(diag);
                    }
                }

                if token.is_env {
                    used_envs.insert(token.name.clone());
                    if !declared_envs.contains(token.name.as_str()) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None)
                            .or_else(|| SourceLocation::find_in_source(input, &format!("{{env:{}}}", token.name), None));
                        let mut diag = Diagnostic::error(
                            "E0021",
                            format!(
                                "Prompt '{}' references undeclared environment variable '{{env:{}}}'",
                                prompt.name, token.name
                            ),
                        )
                        .with_help(format!(
                            "Declare '{0} default=\"...\"' in the top-level 'env {{ ... }}' block.",
                            token.name
                        ));
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("undeclared variable '{{env:{}}}'", token.name));
                        }
                        if let Some(cand) = find_closest_match(&token.name, declared_envs.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{{env:{}}}'?", cand));
                        }
                        report.add(diag);
                    }
                } else {
                    if !declared_args.contains(token.name.as_str()) {
                        let loc = SourceLocation::find_in_source(input, &token.raw, None)
                            .or_else(|| SourceLocation::find_in_source(input, &format!("{{{}}}", token.name), None));
                        let mut diag = Diagnostic::error(
                            "E0022",
                            format!(
                                "Prompt '{}' message references undeclared argument '{{{}}}'",
                                prompt.name, token.name
                            ),
                        )
                        .with_help(format!(
                            "Declare 'argument \"{}\"' in prompt '{}' or check for typos.",
                            token.name, prompt.name
                        ));
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("undeclared argument '{{{}}}'", token.name));
                        }
                        if let Some(cand) = find_closest_match(&token.name, declared_args.iter().copied()) {
                            diag = diag.with_note(format!("Did you mean '{{{}}}'?", cand));
                        }
                        report.add(diag);
                    }
                }
            }
        }
    }

    for template in &server.resource_templates {
        let declared_params: std::collections::HashSet<&str> =
            template.params.iter().map(|p| p.name.as_str()).collect();
        let tokens = parse_placeholder_tokens(&template.uri_template);
        for token in tokens {
            if let Some(m) = &token.modifier {
                if !is_valid_modifier(m) {
                    let loc = SourceLocation::find_in_source(input, &token.raw, None);
                    let mut diag = Diagnostic::error(
                        "E0025",
                        format!(
                            "Unknown placeholder modifier '{}' in '{}'",
                            m, token.raw
                        ),
                    )
                    .with_help("Allowed modifiers are: :json (serialize valid JSON value), :url (percent-encode).");

                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label(format!("unknown modifier ':{}'", m));
                    }

                    if let Some(cand) = find_closest_match(m, ["json", "url"].iter().copied()) {
                        diag = diag.with_note(format!("Did you mean ':{}'?", cand));
                    }

                    report.add(diag);
                }
            }

            if token.is_env {
                used_envs.insert(token.name.clone());
                if !declared_envs.contains(token.name.as_str()) {
                    let loc = SourceLocation::find_in_source(input, &token.raw, None)
                        .or_else(|| SourceLocation::find_in_source(input, &format!("{{env:{}}}", token.name), None));
                    let mut diag = Diagnostic::error(
                        "E0021",
                        format!(
                            "Resource template '{}' references undeclared environment variable '{{env:{}}}'",
                            template.uri_template, token.name
                        ),
                    )
                    .with_help(format!(
                        "Declare '{0} default=\"...\"' in the top-level 'env {{ ... }}' block.",
                        token.name
                    ));
                    if let Some(l) = loc {
                        diag = diag.with_location(l);
                    }
                    report.add(diag);
                }
            } else {
                if !declared_params.contains(token.name.as_str()) {
                    let loc = SourceLocation::find_in_source(input, &token.raw, None)
                        .or_else(|| SourceLocation::find_in_source(input, &format!("{{{}}}", token.name), None));
                    let mut diag = Diagnostic::error(
                        "E0023",
                        format!(
                            "Resource template '{}' URI references undeclared parameter '{{{}}}'",
                            template.uri_template, token.name
                        ),
                    )
                    .with_help(format!(
                        "Declare 'param \"{}\"' on resource template '{}'.",
                        token.name, template.uri_template
                    ));
                    if let Some(l) = loc {
                        diag = diag.with_location(l);
                    }
                    report.add(diag);
                }
            }
        }
    }

    // Unused environment variables: W0002
    for env in &server.envs {
        if !used_envs.contains(&env.name) {
            let loc = SourceLocation::find_in_source(input, &env.name, None);
            let mut diag = Diagnostic::warning(
                "W0002",
                format!("Unused environment variable '{}' declared in env block", env.name),
            )
            .with_help(format!(
                "Remove '{}' from 'env {{ ... }}' or reference it in a tool, resource, or prompt via '{{env:{}}}'.",
                env.name, env.name
            ));
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label("environment variable declared but never used");
            }
            report.add(diag);
        }
    }
}
