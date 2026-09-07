use crate::ast::*;
use crate::diagnostics::{find_closest_match, Diagnostic, DiagnosticReport, SourceLocation};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ProfileCatalog;

impl ProfileCatalog {
    pub fn all_builtin_names() -> &'static [&'static str] {
        &[
            "cdp-chrome",
            "chrome-cdp",
            "cdp",
            "ida-pro",
            "ida",
            "ida-rpc",
            "redis-tcp",
            "redis",
            "rest-json",
            "rest",
            "http-json",
            "json-rpc-http",
            "jsonrpc-http",
            "jsonrpc",
            "json-rpc-ws",
            "json-rpc-pipe",
            "wscript",
            "wscript-shell",
        ]
    }

    pub fn is_builtin(name: &str) -> bool {
        Self::all_builtin_names().contains(&name)
    }

    pub fn get_builtin(name: &str) -> Option<ProfileSpec> {
        match name {
            "cdp-chrome" | "chrome-cdp" | "cdp" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("Chrome DevTools Protocol over WebSocket".to_string()),
                extends: None,
                binding: Some(ToolBinding::Ws(WsBinding {
                    url: Some("ws://127.0.0.1:9222/devtools/browser".to_string()),
                    host: None,
                    port: None,
                    endpoint: None,
                    headers: Vec::new(),
                    message: String::new(),
                    timeout_ms: 10_000,
                    extract_json: Some("$.result".to_string()),
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Json,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: Some("$.result".to_string()),
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: None,
                url: Some("ws://127.0.0.1:9222/devtools/browser".to_string()),
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: None,
                framing: None,
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: Some("$.result".to_string()),
                timeout_ms: Some(10_000),
            }),
            "ida-pro" | "ida" | "ida-rpc" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("IDA Pro RPC server over TCP socket".to_string()),
                extends: None,
                binding: Some(ToolBinding::Pipe(PipeBinding {
                    host: Some("127.0.0.1".to_string()),
                    port: Some("8888".to_string()),
                    addr: None,
                    message: String::new(),
                    framing: "\n".to_string(),
                    timeout_ms: 10_000,
                    extract_json: Some("$.result".to_string()),
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Json,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: Some("$.result".to_string()),
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: None,
                url: None,
                host: Some("127.0.0.1".to_string()),
                port: Some("8888".to_string()),
                command: None,
                args: Vec::new(),
                progid: None,
                framing: Some("\n".to_string()),
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: Some("$.result".to_string()),
                timeout_ms: Some(10_000),
            }),
            "redis-tcp" | "redis" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("Redis command execution over TCP stream".to_string()),
                extends: None,
                binding: Some(ToolBinding::Pipe(PipeBinding {
                    host: Some("127.0.0.1".to_string()),
                    port: Some("6379".to_string()),
                    addr: None,
                    message: String::new(),
                    framing: "\r\n".to_string(),
                    timeout_ms: 10_000,
                    extract_json: None,
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Text,
                    mime_type: Some("text/plain".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: None,
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: None,
                url: None,
                host: Some("127.0.0.1".to_string()),
                port: Some("6379".to_string()),
                command: None,
                args: Vec::new(),
                progid: None,
                framing: Some("\r\n".to_string()),
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: None,
                timeout_ms: Some(10_000),
            }),
            "rest-json" | "rest" | "http-json" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("REST HTTP client with JSON payload framing".to_string()),
                extends: None,
                binding: Some(ToolBinding::Http(HttpBinding {
                    method: "GET".to_string(),
                    url: String::new(),
                    headers: vec![
                        ("Content-Type".to_string(), "application/json".to_string()),
                        ("Accept".to_string(), "application/json".to_string()),
                    ],
                    query: Vec::new(),
                    body: None,
                    timeout_ms: 10_000,
                    extract_json: None,
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Text,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: None,
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: Some("GET".to_string()),
                url: None,
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: None,
                framing: None,
                message: None,
                headers: vec![
                    ("Content-Type".to_string(), "application/json".to_string()),
                    ("Accept".to_string(), "application/json".to_string()),
                ],
                params: Vec::new(),
                extract_json: None,
                timeout_ms: Some(10_000),
            }),
            "json-rpc-http" | "jsonrpc-http" | "jsonrpc" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("JSON-RPC 2.0 over HTTP POST".to_string()),
                extends: None,
                binding: Some(ToolBinding::Http(HttpBinding {
                    method: "POST".to_string(),
                    url: String::new(),
                    headers: vec![
                        ("Content-Type".to_string(), "application/json".to_string()),
                        ("Accept".to_string(), "application/json".to_string()),
                    ],
                    query: Vec::new(),
                    body: None,
                    timeout_ms: 10_000,
                    extract_json: Some("$.result".to_string()),
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Json,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: Some("$.result".to_string()),
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: Some("POST".to_string()),
                url: None,
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: None,
                framing: None,
                message: None,
                headers: vec![
                    ("Content-Type".to_string(), "application/json".to_string()),
                    ("Accept".to_string(), "application/json".to_string()),
                ],
                params: Vec::new(),
                extract_json: Some("$.result".to_string()),
                timeout_ms: Some(10_000),
            }),
            "json-rpc-ws" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("JSON-RPC 2.0 over WebSocket".to_string()),
                extends: None,
                binding: Some(ToolBinding::Ws(WsBinding {
                    url: None,
                    host: None,
                    port: None,
                    endpoint: None,
                    headers: Vec::new(),
                    message: String::new(),
                    timeout_ms: 10_000,
                    extract_json: Some("$.result".to_string()),
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Json,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: Some("$.result".to_string()),
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: None,
                url: None,
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: None,
                framing: None,
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: Some("$.result".to_string()),
                timeout_ms: Some(10_000),
            }),
            "json-rpc-pipe" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("JSON-RPC 2.0 over TCP socket stream".to_string()),
                extends: None,
                binding: Some(ToolBinding::Pipe(PipeBinding {
                    host: None,
                    port: None,
                    addr: None,
                    message: String::new(),
                    framing: "\n".to_string(),
                    timeout_ms: 10_000,
                    extract_json: Some("$.result".to_string()),
                })),
                output: Some(OutputSpec {
                    format: OutputFormat::Json,
                    mime_type: Some("application/json".to_string()),
                    trim: true,
                    slice: None,
                    extract_json: Some("$.result".to_string()),
                    regex: None,
                    filter_not: None,
                    filter: None,
                }),
                method: None,
                url: None,
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: None,
                framing: Some("\n".to_string()),
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: Some("$.result".to_string()),
                timeout_ms: Some(10_000),
            }),
            "wscript" | "wscript-shell" => Some(ProfileSpec {
                name: name.to_string(),
                description: Some("Windows Script Host Shell COM Automation".to_string()),
                extends: None,
                binding: Some(ToolBinding::Com(ComBinding {
                    progid: "WScript.Shell".to_string(),
                    method: String::new(),
                    args: Vec::new(),
                    attach: true,
                    bring_to_front: false,
                    timeout_ms: 10_000,
                })),
                output: Some(OutputSpec::default()),
                method: None,
                url: None,
                host: None,
                port: None,
                command: None,
                args: Vec::new(),
                progid: Some("WScript.Shell".to_string()),
                framing: None,
                message: None,
                headers: Vec::new(),
                params: Vec::new(),
                extract_json: None,
                timeout_ms: Some(10_000),
            }),
            _ => None,
        }
    }
}

pub struct ProfileResolver;

impl ProfileResolver {
    /// Resolve all user-defined and built-in profiles, expanding tools and checking constraints.
    pub fn resolve(
        server: &mut ServerSpec,
        input: &str,
        report: &mut DiagnosticReport,
    ) {
        let mut user_profiles: HashMap<String, ProfileSpec> = HashMap::new();
        for p in &server.profiles {
            user_profiles.insert(p.name.clone(), p.clone());
        }

        // Check for inheritance cycles and resolve full profile hierarchy
        let mut flattened_profiles: HashMap<String, ProfileSpec> = HashMap::new();
        for (name, spec) in &user_profiles {
            let mut visited = HashSet::new();
            if let Some(flat) = Self::flatten_profile(spec, &user_profiles, &mut visited, input, report) {
                flattened_profiles.insert(name.clone(), flat);
            }
        }

        // Resolve default server profile if specified
        if let Some(ref def_name) = server.default_profile {
            if !ProfileCatalog::is_builtin(def_name) && !flattened_profiles.contains_key(def_name) {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", def_name), None)
                    .or_else(|| SourceLocation::find_node_name(input, "profile", None));
                let all_names = Self::available_profile_names(&flattened_profiles);
                let mut diag = Diagnostic::error(
                    "E0065",
                    format!("Server references unknown default profile '{}'", def_name),
                )
                .with_help("Verify the profile name exists in the catalog or document.");
                if let Some(closest) = find_closest_match(def_name, all_names.iter().map(|s| s.as_str())) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown profile '{}'", def_name));
                }
                report.add(diag);
            }
        }

        // Expand profiles on all tools
        for tool in &mut server.tools {
            let tool_offset = SourceLocation::find_in_source(input, &format!("\"{}\"", tool.name), None)
                .or_else(|| SourceLocation::find_node_name(input, "tool", None))
                .map(|l| l.offset);

            // Determine if tool uses explicit profile or inherits default
            let profile_ref = if let Some(ref p) = tool.profile {
                Some(p.clone())
            } else if tool.binding.is_none() {
                server.default_profile.as_ref().map(|def| ToolProfileRef {
                    name: def.clone(),
                    ..Default::default()
                })
            } else {
                None
            };

            if let Some(p_ref) = profile_ref {
                // If tool also has explicit binding, emit error E0054
                if tool.binding.is_some() && tool.profile.is_some() {
                    let loc = SourceLocation::find_node_name(input, "profile", tool_offset);
                    let mut diag = Diagnostic::error(
                        "E0054",
                        format!("Tool '{}' defines both a profile and an explicit execution binding", tool.name),
                    )
                    .with_help("A tool must specify either an execution binding (e.g. bind:exec, bind:http) or a profile, not both.");
                    if let Some(l) = loc {
                        diag = diag.with_location(l).with_label("conflicting profile and binding");
                    }
                    report.add(diag);
                    continue;
                }

                // Look up profile
                let resolved_profile = if let Some(user_p) = flattened_profiles.get(&p_ref.name) {
                    Some(user_p.clone())
                } else if let Some(builtin_p) = ProfileCatalog::get_builtin(&p_ref.name) {
                    Some(builtin_p)
                } else {
                    None
                };

                match resolved_profile {
                    Some(base_profile) => {
                        let concrete_binding = Self::expand_tool_binding(&tool.name, &tool.params, &base_profile, &p_ref);
                        tool.binding = Some(concrete_binding);

                        // If tool output has default format/mime and profile provides output, merge it
                        if let Some(ref prof_output) = base_profile.output {
                            if tool.output.extract_json.is_none() && prof_output.extract_json.is_some() {
                                tool.output.extract_json = prof_output.extract_json.clone();
                            }
                            if tool.output.format == OutputFormat::Text && prof_output.format != OutputFormat::Text {
                                tool.output.format = prof_output.format;
                                tool.output.mime_type = prof_output.mime_type.clone();
                            }
                        }
                    }
                    None => {
                        let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", p_ref.name), tool_offset)
                            .or_else(|| SourceLocation::find_node_name(input, "profile", tool_offset));
                        let all_names = Self::available_profile_names(&flattened_profiles);
                        let mut diag = Diagnostic::error(
                            "E0065",
                            format!("Tool '{}' references unknown profile '{}'", tool.name, p_ref.name),
                        )
                        .with_help("Ensure the profile is defined at top level or is a built-in profile (e.g. cdp-chrome, ida-pro, redis-tcp, rest-json).");
                        if let Some(closest) = find_closest_match(&p_ref.name, all_names.iter().map(|s| s.as_str())) {
                            diag = diag.with_note(format!("Did you mean '{}'?", closest));
                        }
                        if let Some(l) = loc {
                            diag = diag.with_location(l).with_label(format!("unknown profile '{}'", p_ref.name));
                        }
                        report.add(diag);
                    }
                }
            }
        }
    }

    fn available_profile_names(user_profiles: &HashMap<String, ProfileSpec>) -> Vec<String> {
        let mut names: Vec<String> = ProfileCatalog::all_builtin_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        for k in user_profiles.keys() {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
        names
    }

    fn flatten_profile(
        spec: &ProfileSpec,
        user_profiles: &HashMap<String, ProfileSpec>,
        visited: &mut HashSet<String>,
        input: &str,
        report: &mut DiagnosticReport,
    ) -> Option<ProfileSpec> {
        if visited.contains(&spec.name) {
            let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", spec.name), None)
                .or_else(|| SourceLocation::find_node_name(input, "profile", None));
            let mut diag = Diagnostic::error(
                "E0066",
                format!("Circular inheritance detected in profile '{}'", spec.name),
            )
            .with_help("Remove cyclical extends relationships between profiles.");
            if let Some(l) = loc {
                diag = diag.with_location(l).with_label("circular inheritance cycle");
            }
            report.add(diag);
            return None;
        }

        visited.insert(spec.name.clone());

        let mut parent_spec = if let Some(ref parent_name) = spec.extends {
            if let Some(p) = user_profiles.get(parent_name) {
                Self::flatten_profile(p, user_profiles, visited, input, report)
            } else if let Some(builtin) = ProfileCatalog::get_builtin(parent_name) {
                Some(builtin)
            } else {
                let loc = SourceLocation::find_in_source(input, &format!("\"{}\"", parent_name), None)
                    .or_else(|| SourceLocation::find_node_name(input, "extends", None));
                let all_names = Self::available_profile_names(user_profiles);
                let mut diag = Diagnostic::error(
                    "E0065",
                    format!("Profile '{}' extends unknown profile '{}'", spec.name, parent_name),
                )
                .with_help("Verify the base profile exists in the catalog or document.");
                if let Some(closest) = find_closest_match(parent_name, all_names.iter().map(|s| s.as_str())) {
                    diag = diag.with_note(format!("Did you mean '{}'?", closest));
                }
                if let Some(l) = loc {
                    diag = diag.with_location(l).with_label(format!("unknown base profile '{}'", parent_name));
                }
                report.add(diag);
                None
            }
        } else {
            None
        };

        visited.remove(&spec.name);

        let mut result = parent_spec.take().unwrap_or_else(|| ProfileSpec {
            name: spec.name.clone(),
            description: None,
            extends: None,
            binding: None,
            output: None,
            method: None,
            url: None,
            host: None,
            port: None,
            command: None,
            args: Vec::new(),
            progid: None,
            framing: None,
            message: None,
            headers: Vec::new(),
            params: Vec::new(),
            extract_json: None,
            timeout_ms: None,
        });

        result.name = spec.name.clone();
        if spec.description.is_some() {
            result.description = spec.description.clone();
        }
        if let Some(ref child_b) = spec.binding {
            match (&mut result.binding, child_b) {
                (Some(ToolBinding::Http(ref mut p_http)), ToolBinding::Http(c_http)) => {
                    if !c_http.url.is_empty() {
                        p_http.url = c_http.url.clone();
                    }
                    if c_http.method != "GET" || p_http.method.is_empty() {
                        p_http.method = c_http.method.clone();
                    }
                    for (k, v) in &c_http.headers {
                        if !p_http.headers.iter().any(|(pk, _)| pk.eq_ignore_ascii_case(k)) {
                            p_http.headers.push((k.clone(), v.clone()));
                        } else {
                            for existing in &mut p_http.headers {
                                if existing.0.eq_ignore_ascii_case(k) {
                                    existing.1 = v.clone();
                                }
                            }
                        }
                    }
                    if !c_http.query.is_empty() {
                        p_http.query.extend(c_http.query.clone());
                    }
                    if c_http.body.is_some() {
                        p_http.body = c_http.body.clone();
                    }
                    if c_http.timeout_ms != 10_000 {
                        p_http.timeout_ms = c_http.timeout_ms;
                    }
                    if c_http.extract_json.is_some() {
                        p_http.extract_json = c_http.extract_json.clone();
                    }
                }
                (Some(ToolBinding::Ws(ref mut p_ws)), ToolBinding::Ws(c_ws)) => {
                    if c_ws.url.is_some() {
                        p_ws.url = c_ws.url.clone();
                    }
                    if c_ws.host.is_some() {
                        p_ws.host = c_ws.host.clone();
                    }
                    if c_ws.port.is_some() {
                        p_ws.port = c_ws.port.clone();
                    }
                    if c_ws.endpoint.is_some() {
                        p_ws.endpoint = c_ws.endpoint.clone();
                    }
                    for (k, v) in &c_ws.headers {
                        if !p_ws.headers.iter().any(|(pk, _)| pk.eq_ignore_ascii_case(k)) {
                            p_ws.headers.push((k.clone(), v.clone()));
                        }
                    }
                    if !c_ws.message.is_empty() {
                        p_ws.message = c_ws.message.clone();
                    }
                    if c_ws.timeout_ms != 10_000 {
                        p_ws.timeout_ms = c_ws.timeout_ms;
                    }
                    if c_ws.extract_json.is_some() {
                        p_ws.extract_json = c_ws.extract_json.clone();
                    }
                }
                (Some(ToolBinding::Pipe(ref mut p_pipe)), ToolBinding::Pipe(c_pipe)) => {
                    if c_pipe.host.is_some() {
                        p_pipe.host = c_pipe.host.clone();
                    }
                    if c_pipe.port.is_some() {
                        p_pipe.port = c_pipe.port.clone();
                    }
                    if c_pipe.addr.is_some() {
                        p_pipe.addr = c_pipe.addr.clone();
                    }
                    if !c_pipe.message.is_empty() {
                        p_pipe.message = c_pipe.message.clone();
                    }
                    if c_pipe.framing != "\n" {
                        p_pipe.framing = c_pipe.framing.clone();
                    }
                    if c_pipe.timeout_ms != 10_000 {
                        p_pipe.timeout_ms = c_pipe.timeout_ms;
                    }
                    if c_pipe.extract_json.is_some() {
                        p_pipe.extract_json = c_pipe.extract_json.clone();
                    }
                }
                (Some(ToolBinding::Com(ref mut p_com)), ToolBinding::Com(c_com)) => {
                    if !c_com.progid.is_empty() {
                        p_com.progid = c_com.progid.clone();
                    }
                    if !c_com.method.is_empty() {
                        p_com.method = c_com.method.clone();
                    }
                    if !c_com.args.is_empty() {
                        p_com.args = c_com.args.clone();
                    }
                    if c_com.timeout_ms != 10_000 {
                        p_com.timeout_ms = c_com.timeout_ms;
                    }
                }
                (Some(ToolBinding::Exec(ref mut p_exec)), ToolBinding::Exec(c_exec)) => {
                    if !c_exec.command.is_empty() {
                        p_exec.command = c_exec.command.clone();
                    }
                    if !c_exec.args.is_empty() {
                        p_exec.args = c_exec.args.clone();
                    }
                    if c_exec.workdir.is_some() {
                        p_exec.workdir = c_exec.workdir.clone();
                    }
                    if c_exec.timeout_ms != 10_000 {
                        p_exec.timeout_ms = c_exec.timeout_ms;
                    }
                    for cv in &c_exec.variants {
                        if let Some(pos) = p_exec.variants.iter().position(|v| v.os == cv.os) {
                            p_exec.variants[pos] = cv.clone();
                        } else {
                            p_exec.variants.push(cv.clone());
                        }
                    }
                }
                _ => {
                    result.binding = Some(child_b.clone());
                }
            }
        }
        if spec.output.is_some() {
            result.output = spec.output.clone();
        }
        if spec.method.is_some() {
            result.method = spec.method.clone();
        }
        if spec.url.is_some() {
            result.url = spec.url.clone();
        }
        if spec.host.is_some() {
            result.host = spec.host.clone();
        }
        if spec.port.is_some() {
            result.port = spec.port.clone();
        }
        if spec.command.is_some() {
            result.command = spec.command.clone();
        }
        if !spec.args.is_empty() {
            result.args = spec.args.clone();
        }
        if spec.progid.is_some() {
            result.progid = spec.progid.clone();
        }
        if spec.framing.is_some() {
            result.framing = spec.framing.clone();
        }
        if spec.message.is_some() {
            result.message = spec.message.clone();
        }
        if !spec.headers.is_empty() {
            for h in &spec.headers {
                if !result.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(&h.0)) {
                    result.headers.push(h.clone());
                } else {
                    for existing in &mut result.headers {
                        if existing.0.eq_ignore_ascii_case(&h.0) {
                            existing.1 = h.1.clone();
                        }
                    }
                }
            }
        }
        if !spec.params.is_empty() {
            result.params.extend(spec.params.clone());
        }
        if spec.extract_json.is_some() {
            result.extract_json = spec.extract_json.clone();
        }
        if spec.timeout_ms.is_some() {
            result.timeout_ms = spec.timeout_ms;
        }

        Some(result)
    }

    fn expand_tool_binding(
        _tool_name: &str,
        tool_params: &[ParamSpec],
        profile: &ProfileSpec,
        tool_ref: &ToolProfileRef,
    ) -> ToolBinding {
        let base_binding = profile.binding.clone().unwrap_or_else(|| {
            if profile.progid.is_some() || tool_ref.progid.is_some() {
                ToolBinding::Com(ComBinding::default())
            } else if profile.url.as_ref().map(|u| u.starts_with("ws:") || u.starts_with("wss:")).unwrap_or(false)
                || tool_ref.url.as_ref().map(|u| u.starts_with("ws:") || u.starts_with("wss:")).unwrap_or(false)
            {
                ToolBinding::Ws(WsBinding::default())
            } else if profile.host.is_some() || profile.port.is_some() || profile.framing.is_some()
                || tool_ref.host.is_some() || tool_ref.port.is_some()
            {
                ToolBinding::Pipe(PipeBinding::default())
            } else if profile.command.is_some() || tool_ref.command.is_some() {
                ToolBinding::Exec(ExecBinding::default())
            } else {
                ToolBinding::Http(HttpBinding::default())
            }
        });

        match base_binding {
            ToolBinding::Ws(mut ws) => {
                if let Some(u) = tool_ref.url.as_ref().or(profile.url.as_ref()) {
                    ws.url = Some(u.to_string());
                }
                if let Some(h) = tool_ref.host.as_ref().or(profile.host.as_ref()) {
                    ws.host = Some(h.to_string());
                }
                if let Some(p) = tool_ref.port.as_ref().or(profile.port.as_ref()) {
                    ws.port = Some(p.to_string());
                }
                if let Some(ms) = tool_ref.timeout_ms.or(profile.timeout_ms) {
                    ws.timeout_ms = ms;
                }
                if let Some(ej) = tool_ref.extract_json.as_ref().or(profile.extract_json.as_ref()) {
                    ws.extract_json = Some(ej.to_string());
                }

                // Headers
                for ph in &profile.headers {
                    if !ws.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(&ph.0)) {
                        ws.headers.push(ph.clone());
                    }
                }
                for th in &tool_ref.headers {
                    if !ws.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(&th.0)) {
                        ws.headers.push(th.clone());
                    } else {
                        for existing in &mut ws.headers {
                            if existing.0.eq_ignore_ascii_case(&th.0) {
                                existing.1 = th.1.clone();
                            }
                        }
                    }
                }

                // Message synthesis
                if let Some(m) = tool_ref.message.as_ref().or(profile.message.as_ref()) {
                    ws.message = m.to_string();
                } else if let Some(method) = tool_ref.method.as_ref().or(profile.method.as_ref()) {
                    ws.message = Self::synthesize_jsonrpc_message(method, tool_params, &tool_ref.params, false);
                }

                ToolBinding::Ws(ws)
            }

            ToolBinding::Pipe(mut pipe) => {
                if let Some(h) = tool_ref.host.as_ref().or(profile.host.as_ref()) {
                    pipe.host = Some(h.to_string());
                }
                if let Some(p) = tool_ref.port.as_ref().or(profile.port.as_ref()) {
                    pipe.port = Some(p.to_string());
                }
                if let Some(f) = tool_ref.framing.as_ref().or(profile.framing.as_ref()) {
                    pipe.framing = f.to_string();
                }
                if let Some(ms) = tool_ref.timeout_ms.or(profile.timeout_ms) {
                    pipe.timeout_ms = ms;
                }
                if let Some(ej) = tool_ref.extract_json.as_ref().or(profile.extract_json.as_ref()) {
                    pipe.extract_json = Some(ej.to_string());
                }

                if let Some(m) = tool_ref.message.as_ref().or(profile.message.as_ref()) {
                    pipe.message = m.to_string();
                } else if let Some(cmd) = tool_ref.command.as_ref().or(profile.command.as_ref()) {
                    let mut parts: Vec<String> = vec![cmd.to_string()];
                    if !tool_ref.args.is_empty() {
                        parts.extend(tool_ref.args.clone());
                    } else if !profile.args.is_empty() {
                        parts.extend(profile.args.clone());
                    } else {
                        for p in tool_params {
                            parts.push(format!("${}", p.name));
                        }
                    }
                    let mut msg = parts.join(" ");
                    if !msg.ends_with('\n') && !msg.ends_with('\r') {
                        msg.push_str(&pipe.framing);
                    }
                    pipe.message = msg;
                } else if let Some(method) = tool_ref.method.as_ref().or(profile.method.as_ref()) {
                    let mut msg = Self::synthesize_jsonrpc_message(method, tool_params, &tool_ref.params, false);
                    if !msg.ends_with('\n') && !msg.ends_with('\r') {
                        msg.push_str(&pipe.framing);
                    }
                    pipe.message = msg;
                }

                ToolBinding::Pipe(pipe)
            }

            ToolBinding::Http(mut http) => {
                if let Some(u) = tool_ref.url.as_ref().or(profile.url.as_ref()) {
                    http.url = u.to_string();
                }
                if let Some(m) = tool_ref.method.as_ref().or(profile.method.as_ref()) {
                    http.method = m.to_uppercase();
                }
                if let Some(ms) = tool_ref.timeout_ms.or(profile.timeout_ms) {
                    http.timeout_ms = ms;
                }
                if let Some(ej) = tool_ref.extract_json.as_ref().or(profile.extract_json.as_ref()) {
                    http.extract_json = Some(ej.to_string());
                }

                for ph in &profile.headers {
                    if !http.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(&ph.0)) {
                        http.headers.push(ph.clone());
                    }
                }
                for th in &tool_ref.headers {
                    if !http.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(&th.0)) {
                        http.headers.push(th.clone());
                    } else {
                        for existing in &mut http.headers {
                            if existing.0.eq_ignore_ascii_case(&th.0) {
                                existing.1 = th.1.clone();
                            }
                        }
                    }
                }

                if let Some(b) = tool_ref.message.as_ref().or(profile.message.as_ref()) {
                    http.body = Some(b.to_string());
                } else if let Some(method) = tool_ref.method.as_ref().or(profile.method.as_ref()) {
                    if http.method == "POST" || http.method == "PUT" || http.method == "PATCH" {
                        http.body = Some(Self::synthesize_jsonrpc_message(method, tool_params, &tool_ref.params, false));
                    }
                }

                ToolBinding::Http(http)
            }

            ToolBinding::Com(mut com) => {
                if let Some(p) = tool_ref.progid.as_ref().or(profile.progid.as_ref()) {
                    com.progid = p.to_string();
                }
                if let Some(m) = tool_ref.method.as_ref().or(profile.method.as_ref()) {
                    com.method = m.to_string();
                }
                if !tool_ref.args.is_empty() {
                    com.args = tool_ref.args.clone();
                } else if !profile.args.is_empty() {
                    com.args = profile.args.clone();
                } else {
                    for p in tool_params {
                        com.args.push(format!("${}", p.name));
                    }
                }
                if let Some(ms) = tool_ref.timeout_ms.or(profile.timeout_ms) {
                    com.timeout_ms = ms;
                }

                ToolBinding::Com(com)
            }

            ToolBinding::Exec(mut exec) => {
                if let Some(c) = tool_ref.command.as_ref().or(profile.command.as_ref()) {
                    exec.command = c.to_string();
                }
                if !tool_ref.args.is_empty() {
                    exec.args = tool_ref.args.clone();
                } else if !profile.args.is_empty() {
                    exec.args = profile.args.clone();
                }
                if let Some(ms) = tool_ref.timeout_ms.or(profile.timeout_ms) {
                    exec.timeout_ms = ms;
                }

                ToolBinding::Exec(exec)
            }

            other => other,
        }
    }

    fn synthesize_jsonrpc_message(
        method: &str,
        tool_params: &[ParamSpec],
        explicit_mappings: &[(String, String)],
        is_array_params: bool,
    ) -> String {
        if is_array_params {
            let mut items = Vec::new();
            if !explicit_mappings.is_empty() {
                for (_, val) in explicit_mappings {
                    items.push(val.clone());
                }
            } else {
                for p in tool_params {
                    items.push(format!("\"${}\"", p.name));
                }
            }
            return format!(
                r#"{{"jsonrpc": "2.0", "id": 1, "method": "{}", "params": [{}]}}"#,
                method,
                items.join(", ")
            );
        }

        let mut param_pairs = Vec::new();
        if !explicit_mappings.is_empty() {
            for (key, val) in explicit_mappings {
                if val.starts_with('{') || val.starts_with('[') || val == "true" || val == "false" || val.parse::<f64>().is_ok() {
                    param_pairs.push(format!(r#""{}": {}"#, key, val));
                } else if val.starts_with('$') {
                    param_pairs.push(format!(r#""{}": "{}""#, key, val));
                } else {
                    param_pairs.push(format!(r#""{}": "{}""#, key, val));
                }
            }
        } else {
            for p in tool_params {
                param_pairs.push(format!(r#""{}": "${}""#, p.name, p.name));
            }
        }

        let params_body = if param_pairs.is_empty() {
            "{}".to_string()
        } else {
            format!("{{{}}}", param_pairs.join(", "))
        };

        format!(
            r#"{{"id": 1, "method": "{}", "params": {}}}"#,
            method, params_body
        )
    }
}
