pub mod com;
pub mod exec;
pub mod http;
pub mod ipc;
pub mod output;
pub mod prompts_resources;
pub mod sse;
pub mod stdio;
pub mod ws_pipe;

pub const HEADER_TEMPLATE: &str = r###"use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::process::{Command, Output, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// -----------------------------------------------------------------------------
// Compile-Time Server Metadata & Pre-Synthesized Schemas
// -----------------------------------------------------------------------------
pub const PROTOCOL_VERSION: &str = "{{ protocol_version }}";
pub const SERVER_NAME: &str = "{{ server_name }}";
pub const SERVER_VERSION: &str = "{{ server_version }}";
pub const SERVER_DESCRIPTION: &str = "{{ server_description }}";

pub const TOOLS_LIST_JSON: &str = r#"{{ tools_list_json }}"#;
pub const RESOURCES_LIST_JSON: &str = r#"{{ resources_list_json }}"#;
pub const PROMPTS_LIST_JSON: &str = r#"{{ prompts_list_json }}"#;

pub struct ServerEnvDef {
    pub name: &'static str,
    pub var_type: &'static str,
    pub required: bool,
    pub default: &'static str,
    pub description: &'static str,
    pub cli_long: &'static str,
    pub cli_short: Option<&'static str>,
}

pub const SERVER_ENVS: &[ServerEnvDef] = &[
{% for env in envs %}
    ServerEnvDef {
        name: "{{ env.name }}",
        var_type: "{{ env.var_type }}",
        required: {% if env.required %}true{% else %}false{% endif %},
        default: r#"{{ env.default }}"#,
        description: r#"{{ env.description }}"#,
        cli_long: "{{ env.cli_long }}",
        cli_short: {% if env.cli_short %}Some("{{ env.cli_short }}"){% else %}None{% endif %},
    },
{% endfor %}
];

pub fn resolve_path_value(raw: &str) -> String {
    let raw_trimmed = raw.trim();
    if raw_trimmed.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(raw_trimmed);
    if p.is_absolute() {
        return raw_trimmed.to_string();
    }
    // 1. Check relative to CWD
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(p);
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }
    // 2. Check relative to current_exe parent
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join(p);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    // Fallback: return path joined with CWD
    if let Ok(cwd) = std::env::current_dir() {
        return cwd.join(p).to_string_lossy().to_string();
    }
    raw_trimmed.to_string()
}

pub fn init_env_defaults() {
    for env in SERVER_ENVS {
        if !env.default.is_empty() && std::env::var(env.name).is_err() {
            let val = if env.var_type == "path" {
                resolve_path_value(env.default)
            } else {
                env.default.to_string()
            };
            std::env::set_var(env.name, val);
        }
    }
}

pub static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// -----------------------------------------------------------------------------
// JSON-RPC 2.0 Types & Protocol Constants
// -----------------------------------------------------------------------------
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[macro_export]
macro_rules! mcp_log {
    ($($arg:tt)*) => {
        if $crate::VERBOSE.load(std::sync::atomic::Ordering::Relaxed) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let total_secs = now.as_secs();
            let millis = now.subsec_millis();
            let hours = (total_secs / 3600) % 24;
            let mins = (total_secs / 60) % 60;
            let secs = total_secs % 60;
            eprintln!("[{:02}:{:02}:{:02}.{:03}] [{}] {}", hours, mins, secs, millis, SERVER_NAME, format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! mcp_debug {
    ($($arg:tt)*) => {
        if $crate::VERBOSE.load(std::sync::atomic::Ordering::Relaxed) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let total_secs = now.as_secs();
            let millis = now.subsec_millis();
            let hours = (total_secs / 3600) % 24;
            let mins = (total_secs / 60) % 60;
            let secs = total_secs % 60;
            eprintln!("[{:02}:{:02}:{:02}.{:03}] [{}][DEBUG] {}", hours, mins, secs, millis, SERVER_NAME, format_args!($($arg)*));
        }
    };
}

// -----------------------------------------------------------------------------
// String & Template Interpolation Engine
// -----------------------------------------------------------------------------
pub fn url_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

pub fn format_arg_value_modifier(val: &Value, modifier: Option<&str>) -> String {
    match modifier {
        Some("json") => serde_json::to_string(val).unwrap_or_else(|_| "null".to_string()),
        Some("url") => match val {
            Value::String(s) => url_encode_component(s),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => url_encode_component(&other.to_string()),
        },
        _ => match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        },
    }
}

pub fn format_env_str_modifier(val_str: &str, modifier: Option<&str>) -> String {
    match modifier {
        Some("json") => serde_json::to_string(val_str).unwrap_or_else(|_| format!("\"{}\"", val_str)),
        Some("url") => url_encode_component(val_str),
        _ => val_str.to_string(),
    }
}

pub fn interpolate_template(template: &str, args: &Value) -> String {
    let mut result = template.to_string();

    if let Some(obj) = args.as_object() {
        for (key, val) in obj {
            let placeholder_brace = format!("{}{}{}", "{", key, "}");
            if result.contains(&placeholder_brace) {
                result = result.replace(&placeholder_brace, &format_arg_value_modifier(val, None));
            }
            let ph_json = format!("{}{}:json{}", "{", key, "}");
            if result.contains(&ph_json) {
                result = result.replace(&ph_json, &format_arg_value_modifier(val, Some("json")));
            }
            let ph_url = format!("{}{}:url{}", "{", key, "}");
            if result.contains(&ph_url) {
                result = result.replace(&ph_url, &format_arg_value_modifier(val, Some("url")));
            }

            let placeholder_dollar = format!("${}", key);
            if result.contains(&placeholder_dollar) {
                result = result.replace(&placeholder_dollar, &format_arg_value_modifier(val, None));
            }
        }
    }

    for env in SERVER_ENVS {
        let dollar_env = format!("${}", env.name);
        let brace_env = format!("{}{}{}", "{env:", env.name, "}");
        let brace_env_json = format!("{}{}:json{}", "{env:", env.name, "}");
        let brace_env_url = format!("{}{}:url{}", "{env:", env.name, "}");

        if result.contains(&dollar_env)
            || result.contains(&brace_env)
            || result.contains(&brace_env_json)
            || result.contains(&brace_env_url)
        {
            let is_path_type = env.var_type == "path";
            let var_val = std::env::var(env.name).unwrap_or_else(|_| {
                if !env.default.is_empty() {
                    env.default.to_string()
                } else {
                    String::new()
                }
            });
            let resolved = if is_path_type {
                resolve_path_value(&var_val)
            } else {
                var_val
            };

            if result.contains(&dollar_env) {
                result = result.replace(&dollar_env, &format_env_str_modifier(&resolved, None));
            }
            if result.contains(&brace_env) {
                result = result.replace(&brace_env, &format_env_str_modifier(&resolved, None));
            }
            if result.contains(&brace_env_json) {
                result = result.replace(&brace_env_json, &format_env_str_modifier(&resolved, Some("json")));
            }
            if result.contains(&brace_env_url) {
                result = result.replace(&brace_env_url, &format_env_str_modifier(&resolved, Some("url")));
            }
        }
    }

    result
}
"###;

pub const DISPATCH_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Tool Response Envelopes & Execution Router
// -----------------------------------------------------------------------------
pub fn tool_error_response(message: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": message.into() }],
        "isError": true
    })
}

pub fn tool_success_response(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": false
    })
}

pub fn tool_image_response(base64_data: &str, mime_type: &str) -> Value {
    json!({
        "content": [{
            "type": "image",
            "data": base64_data,
            "mimeType": mime_type
        }],
        "isError": false
    })
}

pub fn execute_tool(name: &str, args: &Value) -> Value {
    match name {
{% for tool in tools %}
        "{{ tool.name }}" => {
            let mut effective_args = match args.as_object() {
                Some(map) => map.clone(),
                None => serde_json::Map::new(),
            };
{% for def in tool.defaults %}
            if !effective_args.contains_key("{{ def.0 }}") {
                if let Ok(v) = serde_json::from_str::<Value>(r#"{{ def.1 }}"#) {
                    effective_args.insert("{{ def.0 }}".to_string(), v);
                }
            }
{% endfor %}
            let effective_val = Value::Object(effective_args);
            let args = &effective_val;

{% if tool.binding_type == "exec" %}
            let (cmd_template, args_templates, workdir_template, envs_templates, timeout_ms_val) = {
                #[allow(unused_mut, unused_assignments)]
                let mut matched_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;

                if cfg!(windows) {
                    {% for v in tool.exec_variants %}
                    {% if v.os == "windows" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                } else if cfg!(target_os = "linux") {
                    {% for v in tool.exec_variants %}
                    {% if v.os == "linux" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                    if matched_variant.is_none() {
                        {% for v in tool.exec_variants %}
                        {% if v.os == "unix" %}
                        matched_variant = Some((
                            r#"{{ v.command }}"#,
                            vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                            {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                        ));
                        {% endif %}
                        {% endfor %}
                    }
                } else if cfg!(target_os = "macos") {
                    {% for v in tool.exec_variants %}
                    {% if v.os == "macos" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                    if matched_variant.is_none() {
                        {% for v in tool.exec_variants %}
                        {% if v.os == "unix" %}
                        matched_variant = Some((
                            r#"{{ v.command }}"#,
                            vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                            {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                        ));
                        {% endif %}
                        {% endfor %}
                    }
                } else if cfg!(unix) {
                    {% for v in tool.exec_variants %}
                    {% if v.os == "unix" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                }

                if let Some(v) = matched_variant {
                    v
                } else {
                    #[allow(unused_mut, unused_assignments)]
                    let mut fallback_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;
                    {% for v in tool.exec_variants %}
                    {% if v.os == "fallback" %}
                    fallback_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tool.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}

                    if let Some(fb) = fallback_variant {
                        fb
                    } else {
                        {% if tool.exec_command %}
                        (
                            r#"{{ tool.exec_command }}"#,
                            vec![{% for a in tool.exec_args %}r#"{{ a }}"#,{% endfor %}],
                            {% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}{% if tool.exec_workdir %}Some(r#"{{ tool.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in tool.exec_envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {{ tool.exec_timeout_ms }},
                        )
                        {% else %}
                        (
                            "",
                            vec![],
                            None,
                            vec![],
                            10000,
                        )
                        {% endif %}
                    }
                }
            };

            if cmd_template.is_empty() {
                return tool_error_response(format!(
                    "No executable command configured for target OS '{}' on tool '{}'",
                    std::env::consts::OS,
                    "{{ tool.name }}"
                ));
            }

            let cmd_str = interpolate_template(cmd_template, args);
            let raw_args: Vec<String> = args_templates.into_iter().map(|a| interpolate_template(a, args)).collect();
            let workdir_str: Option<String> = workdir_template.map(|w| interpolate_template(w, args));
            let envs: Vec<(String, String)> = envs_templates.into_iter().map(|(k, v)| (k.to_string(), interpolate_template(v, args))).collect();

            let opts = ExecOptions {
                command: &cmd_str,
                args: &raw_args,
                workdir: workdir_str.as_deref(),
                envs: &envs,
                timeout: Duration::from_millis(timeout_ms_val),
            };

            match execute_subprocess(opts) {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    let transformed = match apply_output_pipeline(
                        text,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };

{% if tool.output_format == "image" %}
                    let b64 = base64_encode(transformed.as_bytes());
                    tool_image_response(&b64, "{{ tool.mime_type }}")
{% elif tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Ok(output) => {
                    let err_text = String::from_utf8_lossy(&output.stderr).to_string();
                    let code = output.status.code().unwrap_or(-1);
                    tool_error_response(format!("Command exited with status {}:\n{}", code, err_text))
                }
                Err(err) => tool_error_response(err),
            }
{% elif tool.binding_type == "http" %}
            let raw_url = interpolate_template(r#"{{ tool.http_url }}"#, args);
            let headers: Vec<(String, String)> = vec![
{% for h in tool.http_headers %}
                ("{{ h.0 }}".to_string(), interpolate_template(r#"{{ h.1 }}"#, args)),
{% endfor %}
            ];
            let query: Vec<(String, String)> = vec![
{% for q in tool.http_query %}
                ("{{ q.0 }}".to_string(), interpolate_template(r#"{{ q.1 }}"#, args)),
{% endfor %}
            ];
            let body_str: Option<String> = {% if tool.http_body %}Some(interpolate_template(r#"{{ tool.http_body }}"#, args)){% else %}None{% endif %};
            let extract_field: Option<&str> = {% if tool.http_extract %}Some(r#"{{ tool.http_extract }}"#){% else %}None{% endif %};

            let opts = HttpOptions {
                method: "{{ tool.http_method }}",
                url: &raw_url,
                headers: &headers,
                query: &query,
                body: body_str.as_deref(),
                timeout: Duration::from_millis({{ tool.http_timeout_ms }}),
                extract_json_field: extract_field,
            };

            match execute_http_request(opts) {
                Ok(resp) => {
                    let transformed = match apply_output_pipeline(
                        resp,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };
{% if tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Err(err) => tool_error_response(err),
            }
{% elif tool.binding_type == "ipc" %}
            let dir_str = interpolate_template(r#"{{ tool.ipc_dir }}"#, args);
            let method_str = interpolate_template(r#"{{ tool.ipc_method }}"#, args);
            let params_str: Option<String> = {% if tool.ipc_params %}Some(interpolate_template(r#"{{ tool.ipc_params }}"#, args)){% else %}None{% endif %};

            let opts = IpcOptions {
                dir: &dir_str,
                method: &method_str,
                params: params_str.as_deref(),
                timeout: Duration::from_millis({{ tool.ipc_timeout_ms }}),
            };

            mcp_log!("[TOOL] Executing IPC tool '{}' -> method '{}'", "{{ tool.name }}", method_str);
            let start = std::time::Instant::now();
            match execute_ipc_request(opts) {
                Ok(res) => {
                    mcp_log!("[TOOL] IPC tool '{}' finished in {}ms", "{{ tool.name }}", start.elapsed().as_millis());
                    let raw_str = if let Some(s) = res.as_str() {
                        s.to_string()
                    } else if res.is_number() || res.is_boolean() {
                        res.to_string()
                    } else {
                        serde_json::to_string(&res).unwrap_or_else(|_| res.to_string())
                    };

                    let transformed = match apply_output_pipeline(
                        raw_str,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };

{% if tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Err(err) => {
                    mcp_log!("[TOOL][ERROR] IPC tool '{}' failed: {}", "{{ tool.name }}", err);
                    tool_error_response(err)
                }
            }
{% elif tool.binding_type == "com" %}
            let progid_str = interpolate_template(r#"{{ tool.com_progid }}"#, args);
            let method_str = interpolate_template(r#"{{ tool.com_method }}"#, args);
            let raw_args: Vec<String> = vec![
{% if tool.com_args %}
{% for arg in tool.com_args %}
                interpolate_template(r#"{{ arg }}"#, args),
{% endfor %}
{% endif %}
            ];

            let json_args: Vec<Value> = raw_args
                .into_iter()
                .map(|a| serde_json::from_str::<Value>(&a).unwrap_or_else(|_| Value::String(a)))
                .collect();

            let opts = ComOptions {
                progid: &progid_str,
                method: &method_str,
                args: &json_args,
                attach: {% if tool.com_attach %}true{% else %}false{% endif %},
                bring_to_front: {% if tool.com_bring_to_front %}true{% else %}false{% endif %},
                timeout: Duration::from_millis({{ tool.com_timeout_ms }}),
            };

            mcp_log!("[TOOL] Executing COM tool '{}' -> {}.{}", "{{ tool.name }}", progid_str, method_str);
            let start = std::time::Instant::now();

            match execute_com_request(opts) {
                Ok(res) => {
                    mcp_log!("[TOOL] COM tool '{}' finished in {}ms", "{{ tool.name }}", start.elapsed().as_millis());
                    let raw_str = if let Some(s) = res.as_str() {
                        s.to_string()
                    } else if res.is_number() || res.is_boolean() {
                        res.to_string()
                    } else {
                        serde_json::to_string(&res).unwrap_or_else(|_| res.to_string())
                    };

                    let transformed = match apply_output_pipeline(
                        raw_str,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };

{% if tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Err(err) => {
                    mcp_log!("[TOOL][ERROR] COM tool '{}' failed: {}", "{{ tool.name }}", err);
                    tool_error_response(err)
                }
            }
{% elif tool.binding_type == "ws" %}
            let raw_url: Option<String> = {% if tool.ws_url %}Some(interpolate_template(r#"{{ tool.ws_url }}"#, args)){% else %}None{% endif %};
            let host_str: Option<String> = {% if tool.ws_host %}Some(interpolate_template(r#"{{ tool.ws_host }}"#, args)){% else %}None{% endif %};
            let port_str: Option<String> = {% if tool.ws_port %}Some(interpolate_template(r#"{{ tool.ws_port }}"#, args)){% else %}None{% endif %};
            let endpoint_str: Option<String> = {% if tool.ws_endpoint %}Some(interpolate_template(r#"{{ tool.ws_endpoint }}"#, args)){% else %}None{% endif %};
            let msg_str = interpolate_template(r#"{{ tool.ws_message }}"#, args);
            let headers: Vec<(String, String)> = vec![
{% for h in tool.ws_headers %}
                ("{{ h.0 }}".to_string(), interpolate_template(r#"{{ h.1 }}"#, args)),
{% endfor %}
            ];
            let extract_field: Option<&str> = {% if tool.ws_extract %}Some(r#"{{ tool.ws_extract }}"#){% else %}None{% endif %};

            let opts = WsOptions {
                url: raw_url.as_deref(),
                host: host_str.as_deref(),
                port: port_str.as_deref(),
                endpoint: endpoint_str.as_deref(),
                headers: &headers,
                message: &msg_str,
                timeout: Duration::from_millis({{ tool.ws_timeout_ms }}),
                extract_json_field: extract_field,
            };

            mcp_log!("[TOOL] Executing WebSocket tool '{}'", "{{ tool.name }}");
            let start = std::time::Instant::now();
            match execute_ws_request(opts) {
                Ok(resp) => {
                    mcp_log!("[TOOL] WebSocket tool '{}' finished in {}ms", "{{ tool.name }}", start.elapsed().as_millis());
                    let transformed = match apply_output_pipeline(
                        resp,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };

{% if tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Err(err) => {
                    mcp_log!("[TOOL][ERROR] WebSocket tool '{}' failed: {}", "{{ tool.name }}", err);
                    tool_error_response(err)
                }
            }
{% elif tool.binding_type == "pipe" %}
            let host_str: Option<String> = {% if tool.pipe_host %}Some(interpolate_template(r#"{{ tool.pipe_host }}"#, args)){% else %}None{% endif %};
            let port_str: Option<String> = {% if tool.pipe_port %}Some(interpolate_template(r#"{{ tool.pipe_port }}"#, args)){% else %}None{% endif %};
            let addr_str: Option<String> = {% if tool.pipe_addr %}Some(interpolate_template(r#"{{ tool.pipe_addr }}"#, args)){% else %}None{% endif %};
            let msg_str = interpolate_template(r#"{{ tool.pipe_message }}"#, args);
            let framing_str = interpolate_template(r#"{{ tool.pipe_framing }}"#, args);
            let extract_field: Option<&str> = {% if tool.pipe_extract %}Some(r#"{{ tool.pipe_extract }}"#){% else %}None{% endif %};

            let opts = PipeOptions {
                host: host_str.as_deref(),
                port: port_str.as_deref(),
                addr: addr_str.as_deref(),
                message: &msg_str,
                framing: &framing_str,
                timeout: Duration::from_millis({{ tool.pipe_timeout_ms }}),
                extract_json_field: extract_field,
            };

            mcp_log!("[TOOL] Executing TCP/Pipe tool '{}'", "{{ tool.name }}");
            let start = std::time::Instant::now();
            match execute_pipe_request(opts) {
                Ok(resp) => {
                    mcp_log!("[TOOL] TCP/Pipe tool '{}' finished in {}ms", "{{ tool.name }}", start.elapsed().as_millis());
                    let transformed = match apply_output_pipeline(
                        resp,
                        {% if tool.output_extract_json %}Some(r#"{{ tool.output_extract_json }}"#){% else %}None{% endif %},
                        {% if tool.output_filter_not %}Some(r#"{{ tool.output_filter_not }}"#){% else %}None{% endif %},
                        {% if tool.output_filter %}Some(r#"{{ tool.output_filter }}"#){% else %}None{% endif %},
                        {% if tool.output_slice_lines %}Some(({{ tool.output_slice_lines }}, {% if tool.output_slice_head %}true{% else %}false{% endif %})){% else %}None{% endif %},
                        {% if tool.output_regex_pattern %}Some((r#"{{ tool.output_regex_pattern }}"#, {% if tool.output_regex_template %}Some(r#"{{ tool.output_regex_template }}"#){% else %}None{% endif %})){% else %}None{% endif %},
                        {% if tool.output_trim %}true{% else %}false{% endif %},
                    ) {
                        Ok(t) => t,
                        Err(e) => return tool_error_response(e),
                    };

{% if tool.output_format == "json" %}
                    let formatted = if let Ok(v) = serde_json::from_str::<Value>(&transformed) {
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| transformed)
                    } else {
                        transformed
                    };
                    tool_success_response(formatted)
{% else %}
                    tool_success_response(transformed)
{% endif %}
                }
                Err(err) => {
                    mcp_log!("[TOOL][ERROR] TCP/Pipe tool '{}' failed: {}", "{{ tool.name }}", err);
                    tool_error_response(err)
                }
            }
{% else %}
            tool_success_response(format!("Executed tool '{}' (stub binding)", "{{ tool.name }}"))
{% endif %}
        }
{% endfor %}
        unknown => tool_error_response(format!("Tool '{}' not found", unknown)),
    }
}
"###;

pub const SERVER_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Server Core Dispatcher & Lifecycle Manager
// -----------------------------------------------------------------------------
pub struct McpServer {
    writer: StdoutWriter,
    initialized: bool,
}

impl McpServer {
    pub fn new(writer: StdoutWriter) -> Self {
        Self {
            writer,
            initialized: false,
        }
    }

    pub fn dispatch(&mut self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let method = req.method.as_str();

        mcp_debug!("<- Incoming RPC method: '{}', id: {:?}", method, id);

        match method {
            "initialize" => {
                self.initialized = true;
                Some(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION
                        },
                        "capabilities": {
                            "tools": { "listChanged": false },
                            "resources": { "subscribe": false, "listChanged": false },
                            "prompts": { "listChanged": false },
                            "logging": {}
                        }
                    })),
                    error: None,
                })
            }
            "notifications/initialized" => {
                mcp_log!("Client initialization confirmed");
                None
            }
            "ping" => Some(JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({})),
                error: None,
            }),
            "tools/list" => {
                let tools_val: Value = serde_json::from_str(TOOLS_LIST_JSON).unwrap_or(Value::Array(Vec::new()));
                Some(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({ "tools": tools_val })),
                    error: None,
                })
            }
            "tools/call" => {
                let params = match req.params {
                    Some(Value::Object(p)) => p,
                    _ => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing or invalid 'params' object".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                let name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing required parameter 'name'".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                let arguments = params.get("arguments").unwrap_or(&Value::Null);
                mcp_debug!("Calling tool '{}' with args: {}", name, arguments);
                let result = execute_tool(name, arguments);

                Some(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(result),
                    error: None,
                })
            }
            "resources/list" => {
                let resources_val: Value = serde_json::from_str(RESOURCES_LIST_JSON).unwrap_or(Value::Array(Vec::new()));
                Some(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({ "resources": resources_val })),
                    error: None,
                })
            }
            "resources/read" => {
                let params = match req.params {
                    Some(Value::Object(p)) => p,
                    _ => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing or invalid 'params' object".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                let uri = match params.get("uri").and_then(|v| v.as_str()) {
                    Some(u) => u,
                    None => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing required parameter 'uri'".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                mcp_debug!("Reading resource '{}'", uri);
                match execute_resource_read(uri) {
                    Ok(res) => Some(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: Some(res),
                        error: None,
                    }),
                    Err(err_msg) => Some(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: INVALID_PARAMS,
                            message: err_msg,
                            data: None,
                        }),
                    }),
                }
            }
            "prompts/list" => {
                let prompts_val: Value = serde_json::from_str(PROMPTS_LIST_JSON).unwrap_or(Value::Array(Vec::new()));
                Some(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({ "prompts": prompts_val })),
                    error: None,
                })
            }
            "prompts/get" => {
                let params = match req.params {
                    Some(Value::Object(p)) => p,
                    _ => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing or invalid 'params' object".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                let name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Some(JsonRpcResponse {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: INVALID_PARAMS,
                                message: "Missing required parameter 'name'".to_string(),
                                data: None,
                            }),
                        });
                    }
                };

                let arguments = params.get("arguments").unwrap_or(&Value::Null);
                mcp_debug!("Getting prompt '{}' with args: {}", name, arguments);
                match execute_prompt_get(name, arguments) {
                    Ok(res) => Some(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: Some(res),
                        error: None,
                    }),
                    Err(err_msg) => Some(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: INVALID_PARAMS,
                            message: err_msg,
                            data: None,
                        }),
                    }),
                }
            }
            "logging/setLevel" => Some(JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({})),
                error: None,
            }),
            _ => {
                if id.is_some() {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: METHOD_NOT_FOUND,
                            message: format!("Method '{}' not found", method),
                            data: None,
                        }),
                    })
                } else {
                    None
                }
            }
        }
    }
}
"###;

pub const CLI_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// CLI Argument Parsing, Environment Verification & Binary Entrypoint
// -----------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub transport: String,
    pub host: String,
    pub port: u16,
    pub verbose: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: "stdio".to_string(),
            host: "{{ default_host }}".to_string(),
            port: {{ default_port }},
            verbose: false,
        }
    }
}

pub fn validate_required_envs() -> Result<(), String> {
    for env in SERVER_ENVS {
        if env.required && std::env::var(env.name).is_err() {
            let desc_suffix = if env.description.is_empty() { String::new() } else { format!(" ({})", env.description) };
            return Err(format!(
                "Missing required environment variable '{}'{}. Pass via {} or -e {}=value.",
                env.name, desc_suffix, env.cli_long, env.name
            ));
        }
    }
    Ok(())
}

pub const HELP_TEXT: &str = r#"{{ server_name }} v{{ server_version }}
{{ server_description }}

USAGE:
    {{ server_name_kebab }} [OPTIONS]

STANDARD OPTIONS:
    -h, --help                Show this help information and exit
    -V, --version             Print version information and exit
    -v, --verbose             Enable debug logging to stderr
    -t, --transport <TYPE>    Transport protocol ('stdio' or 'sse') [default: stdio]
    --host <IP>               Host address for network transports [default: {{ default_host }}]
    -p, --port <PORT>         Port for network transports [default: {{ default_port }}]
    -e, --env <KEY=VALUE>     Set runtime environment variable
    --check-env               Validate required environment variables and exit
{% if envs %}
CONFIGURATION OPTIONS:
{% for env in envs %}
    {% if env.cli_short %}{{ env.cli_short }}, {% else %}    {% endif %}{{ env.cli_long }} <VALUE>{% if env.description %}  {{ env.description }}{% endif %} [env: {{ env.name }}]{% if env.default %} [default: "{{ env.default }}"]{% endif %}
{% endfor %}
{% endif %}
CAPABILITIES:
    Protocol Version: {{ protocol_version }}
    Declared Tools ({{ tools | length }}):
{% for t in tools %}
      - {{ t.name }}{% if t.description %}: {{ t.description }}{% endif %}
{% endfor %}
"#;

pub fn print_help() {
    print!("{}", HELP_TEXT);
}

pub fn parse_cli_args() -> Result<Option<ServerConfig>, String> {
    init_env_defaults();
    let mut config = ServerConfig::default();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("{} v{} (MCP {})", SERVER_NAME, SERVER_VERSION, PROTOCOL_VERSION);
                return Ok(None);
            }
            "-v" | "--verbose" => {
                config.verbose = true;
                VERBOSE.store(true, std::sync::atomic::Ordering::Relaxed);
                i += 1;
            }
            "-t" | "--transport" => {
                if i + 1 >= args.len() {
                    return Err("Missing argument for --transport (expected 'stdio' or 'sse')".to_string());
                }
                config.transport = args[i + 1].clone();
                i += 2;
            }
            "--host" => {
                if i + 1 >= args.len() {
                    return Err("Missing argument for --host".to_string());
                }
                config.host = args[i + 1].clone();
                i += 2;
            }
            "-p" | "--port" => {
                if i + 1 >= args.len() {
                    return Err("Missing argument for --port".to_string());
                }
                config.port = args[i + 1].parse::<u16>().map_err(|e| format!("Invalid port number: {}", e))?;
                i += 2;
            }
            "-e" | "--env" => {
                if i + 1 >= args.len() {
                    return Err("Missing argument for --env (expected KEY=VALUE)".to_string());
                }
                let pair = &args[i + 1];
                if let Some((k, v)) = pair.split_once('=') {
                    let is_path = SERVER_ENVS.iter().any(|e| e.name == k && e.var_type == "path");
                    let final_val = if is_path { resolve_path_value(v) } else { v.to_string() };
                    std::env::set_var(k, final_val);
                } else {
                    return Err(format!("Invalid --env format '{}' (expected KEY=VALUE)", pair));
                }
                i += 2;
            }
            "--check-env" => {
                if let Err(e) = validate_required_envs() {
                    eprintln!("[ERROR] Environment validation failed: {}", e);
                    std::process::exit(1);
                }
                println!("✓ All required environment variables are satisfied.");
                return Ok(None);
            }
            other => {
                let mut matched = false;
                for env in SERVER_ENVS {
                    let is_long_match = other == env.cli_long || other == format!("--{}", env.name.to_lowercase().replace('_', "-"));
                    let is_short_match = env.cli_short.map(|s| other == s).unwrap_or(false);

                    if is_long_match || is_short_match {
                        if env.var_type == "boolean" {
                            std::env::set_var(env.name, "true");
                            matched = true;
                            i += 1;
                            break;
                        } else {
                            if i + 1 >= args.len() {
                                return Err(format!("Missing argument for flag '{}'", other));
                            }
                            let raw_val = &args[i + 1];
                            let final_val = if env.var_type == "path" {
                                resolve_path_value(raw_val)
                            } else {
                                raw_val.to_string()
                            };
                            std::env::set_var(env.name, final_val);
                            matched = true;
                            i += 2;
                            break;
                        }
                    }
                }

                if !matched {
                    return Err(format!("Unknown argument '{}'. Use --help for usage instructions.", other));
                }
            }
        }
    }

    Ok(Some(config))
}

fn main() {
    let config = match parse_cli_args() {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return,
        Err(err) => {
            eprintln!("[FATAL] CLI Error: {}", err);
            std::process::exit(1);
        }
    };

    if config.verbose {
        VERBOSE.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    if let Err(err) = validate_required_envs() {
        eprintln!("[FATAL] {}", err);
        std::process::exit(1);
    }

    match config.transport.as_str() {
        "stdio" => {
            let writer = StdoutWriter::new();
            let mut server = McpServer::new(writer);
            if let Err(e) = server.run_stdio() {
                eprintln!("[FATAL] Server terminated with error: {}", e);
                std::process::exit(1);
            }
        }
        "sse" | "http" => {
            let sse_server = SseServer::new(&config.host, config.port);
            if let Err(e) = sse_server.run() {
                eprintln!("[FATAL] SSE Server terminated with error: {}", e);
                std::process::exit(1);
            }
        }
        unknown => {
            eprintln!("[FATAL] Unsupported transport '{}'. Choose 'stdio' or 'sse'.", unknown);
            std::process::exit(1);
        }
    }
}
"###;
