use crate::ast::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledExecVariant {
    pub os: String,
    pub command: String,
    pub args: Vec<String>,
    pub workdir: Option<String>,
    pub timeout_ms: Option<u64>,
    pub capture_stdout: Option<bool>,
    pub capture_stderr: Option<bool>,
    pub envs: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledToolSpec {
    pub name: String,
    pub description: Option<String>,
    pub binding_type: String,
    pub output_format: String,
    pub mime_type: String,
    pub output_trim: bool,
    pub output_slice_lines: Option<usize>,
    pub output_slice_head: bool,
    pub output_extract_json: Option<String>,
    pub output_filter_not: Option<String>,
    pub output_filter: Option<String>,
    pub output_regex_pattern: Option<String>,
    pub output_regex_template: Option<String>,
    pub exec_command: Option<String>,
    pub exec_args: Option<Vec<String>>,
    pub exec_workdir: Option<String>,
    pub exec_timeout_ms: u64,
    pub exec_envs: Vec<(String, String)>,
    pub exec_variants: Vec<CompiledExecVariant>,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub http_method: Option<String>,
    pub http_url: Option<String>,
    pub http_headers: Vec<(String, String)>,
    pub http_query: Vec<(String, String)>,
    pub http_body: Option<String>,
    pub http_timeout_ms: u64,
    pub http_extract: Option<String>,
    pub ipc_dir: Option<String>,
    pub ipc_method: Option<String>,
    pub ipc_params: Option<String>,
    pub ipc_timeout_ms: u64,
    pub com_progid: Option<String>,
    pub com_method: Option<String>,
    pub com_args: Option<Vec<String>>,
    pub com_attach: bool,
    pub com_bring_to_front: bool,
    pub com_timeout_ms: u64,
    pub ws_url: Option<String>,
    pub ws_host: Option<String>,
    pub ws_port: Option<String>,
    pub ws_endpoint: Option<String>,
    pub ws_headers: Vec<(String, String)>,
    pub ws_message: Option<String>,
    pub ws_timeout_ms: u64,
    pub ws_extract: Option<String>,
    pub pipe_host: Option<String>,
    pub pipe_port: Option<String>,
    pub pipe_addr: Option<String>,
    pub pipe_message: Option<String>,
    pub pipe_framing: Option<String>,
    pub pipe_timeout_ms: u64,
    pub pipe_extract: Option<String>,
    pub defaults: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPromptSpec {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<CompiledPromptArgSpec>,
    pub messages: Vec<CompiledPromptMessageSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPromptArgSpec {
    pub name: String,
    pub required: bool,
    pub default: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPromptMessageSpec {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledResourceSpec {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub content_kind: String,
    pub text_content: Option<String>,
    pub binary_content: Option<String>,
    pub exec_command: Option<String>,
    pub exec_args: Option<Vec<String>>,
    pub exec_workdir: Option<String>,
    pub exec_timeout_ms: u64,
    pub exec_envs: Vec<(String, String)>,
    pub exec_variants: Vec<CompiledExecVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledResourceTemplateSpec {
    pub uri_template: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub binding_type: String,
    pub exec_command: Option<String>,
    pub exec_args: Option<Vec<String>>,
    pub exec_workdir: Option<String>,
    pub exec_timeout_ms: u64,
    pub exec_envs: Vec<(String, String)>,
    pub exec_variants: Vec<CompiledExecVariant>,
    pub http_method: Option<String>,
    pub http_url: Option<String>,
    pub http_headers: Vec<(String, String)>,
    pub http_query: Vec<(String, String)>,
    pub http_body: Option<String>,
    pub http_timeout_ms: u64,
    pub http_extract: Option<String>,
    pub defaults: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledEnvSpec {
    pub name: String,
    pub var_type: String,
    pub required: bool,
    pub default: String,
    pub description: String,
    pub cli_long: String,
    pub cli_short: Option<String>,
}

pub fn compile_prompts(prompts: &[PromptSpec]) -> Vec<CompiledPromptSpec> {
    prompts
        .iter()
        .map(|p| CompiledPromptSpec {
            name: p.name.clone(),
            description: p.description.clone(),
            arguments: p
                .arguments
                .iter()
                .map(|a| CompiledPromptArgSpec {
                    name: a.name.clone(),
                    required: a.required,
                    default: a.default.clone(),
                    description: a.description.clone(),
                })
                .collect(),
            messages: p
                .messages
                .iter()
                .map(|m| CompiledPromptMessageSpec {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect(),
        })
        .collect()
}

pub fn compile_resources(resources: &[ResourceSpec]) -> Vec<CompiledResourceSpec> {
    resources
        .iter()
        .map(|r| {
            let mut exec_variants = Vec::new();
            let (
                content_kind,
                text_content,
                binary_content,
                exec_command,
                exec_args,
                exec_workdir,
                exec_timeout_ms,
                exec_envs,
            ) = match &r.content {
                Some(ResourceContent::Text { text }) => (
                    "text".to_string(),
                    Some(text.clone()),
                    None,
                    None,
                    None,
                    None,
                    5000,
                    Vec::new(),
                ),
                Some(ResourceContent::Binary { blob }) => (
                    "binary".to_string(),
                    None,
                    Some(blob.clone()),
                    None,
                    None,
                    None,
                    5000,
                    Vec::new(),
                ),
                Some(ResourceContent::Exec(e)) => {
                    for v in &e.variants {
                        exec_variants.push(CompiledExecVariant {
                            os: v.os.as_str().to_string(),
                            command: v.command.clone(),
                            args: v.args.clone(),
                            workdir: v.workdir.clone(),
                            timeout_ms: v.timeout_ms,
                            capture_stdout: v.capture_stdout,
                            capture_stderr: v.capture_stderr,
                            envs: v.envs.clone(),
                        });
                    }
                    (
                        "exec".to_string(),
                        None,
                        None,
                        if !e.command.is_empty() {
                            Some(e.command.clone())
                        } else {
                            None
                        },
                        Some(e.args.clone()),
                        e.workdir.clone(),
                        e.timeout_ms,
                        e.envs.clone(),
                    )
                }
                Some(ResourceContent::Native { symbol }) => (
                    "native".to_string(),
                    None,
                    None,
                    Some(symbol.clone()),
                    None,
                    None,
                    5000,
                    Vec::new(),
                ),
                None => (
                    "none".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    5000,
                    Vec::new(),
                ),
            };

            CompiledResourceSpec {
                uri: r.uri.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
                mime_type: r.mime_type.clone().unwrap_or_else(|| "text/plain".to_string()),
                content_kind,
                text_content,
                binary_content,
                exec_command,
                exec_args,
                exec_workdir,
                exec_timeout_ms,
                exec_envs,
                exec_variants,
            }
        })
        .collect()
}

pub fn compile_resource_templates(
    templates: &[ResourceTemplateSpec],
) -> Vec<CompiledResourceTemplateSpec> {
    templates
        .iter()
        .map(|rt| {
            let mut exec_variants = Vec::new();
            let (
                binding_type,
                exec_command,
                exec_args,
                exec_workdir,
                exec_timeout_ms,
                exec_envs,
                http_method,
                http_url,
                http_headers,
                http_query,
                http_body,
                http_timeout_ms,
                http_extract,
            ) = match &rt.binding {
                Some(ToolBinding::Exec(e)) => {
                    for v in &e.variants {
                        exec_variants.push(CompiledExecVariant {
                            os: v.os.as_str().to_string(),
                            command: v.command.clone(),
                            args: v.args.clone(),
                            workdir: v.workdir.clone(),
                            timeout_ms: v.timeout_ms,
                            capture_stdout: v.capture_stdout,
                            capture_stderr: v.capture_stderr,
                            envs: v.envs.clone(),
                        });
                    }
                    (
                        "exec".to_string(),
                        if !e.command.is_empty() {
                            Some(e.command.clone())
                        } else {
                            None
                        },
                        Some(e.args.clone()),
                        e.workdir.clone(),
                        e.timeout_ms,
                        e.envs.clone(),
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        None,
                        10000,
                        None,
                    )
                }
                Some(ToolBinding::Http(h)) => (
                    "http".to_string(),
                    None,
                    None,
                    None,
                    10000,
                    Vec::new(),
                    Some(h.method.clone()),
                    Some(h.url.clone()),
                    h.headers.clone(),
                    h.query.clone(),
                    h.body.clone(),
                    h.timeout_ms,
                    h.extract_json.clone(),
                ),
                _ => (
                    "none".to_string(),
                    None,
                    None,
                    None,
                    10000,
                    Vec::new(),
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    None,
                    10000,
                    None,
                ),
            };

            let defaults: Vec<(String, String)> = rt
                .params
                .iter()
                .filter_map(|p| {
                    p.default
                        .as_ref()
                        .map(|d| (p.name.clone(), serde_json::to_string(d).unwrap_or_default()))
                })
                .collect();

            CompiledResourceTemplateSpec {
                uri_template: rt.uri_template.clone(),
                name: rt.name.clone(),
                description: rt.description.clone(),
                mime_type: rt.mime_type.clone().unwrap_or_else(|| "text/plain".to_string()),
                binding_type,
                exec_command,
                exec_args,
                exec_workdir,
                exec_timeout_ms,
                exec_envs,
                exec_variants,
                http_method,
                http_url,
                http_headers,
                http_query,
                http_body,
                http_timeout_ms,
                http_extract,
                defaults,
            }
        })
        .collect()
}

pub fn compile_tools(tools: &[ToolSpec]) -> Vec<CompiledToolSpec> {
    tools
        .iter()
        .map(|t| {
            let mut exec_command = None;
            let mut exec_args = None;
            let mut exec_workdir = None;
            let mut exec_timeout_ms = 10000;
            let mut exec_envs = Vec::new();
            let mut capture_stdout = true;
            let mut capture_stderr = true;
            let mut http_method = None;
            let mut http_url = None;
            let mut http_headers = Vec::new();
            let mut http_query = Vec::new();
            let mut http_body = None;
            let mut http_timeout_ms = 10000;
            let mut http_extract = None;
            let mut ipc_dir = None;
            let mut ipc_method = None;
            let mut ipc_params = None;
            let mut ipc_timeout_ms = 10000;
            let mut com_progid = None;
            let mut com_method = None;
            let mut com_args = None;
            let mut com_attach = true;
            let mut com_bring_to_front = false;
            let mut com_timeout_ms = 10000;
            let mut ws_url = None;
            let mut ws_host = None;
            let mut ws_port = None;
            let mut ws_endpoint = None;
            let mut ws_headers = Vec::new();
            let mut ws_message = None;
            let mut ws_timeout_ms = 10000;
            let mut ws_extract = None;
            let mut pipe_host = None;
            let mut pipe_port = None;
            let mut pipe_addr = None;
            let mut pipe_message = None;
            let mut pipe_framing = None;
            let mut pipe_timeout_ms = 10000;
            let mut pipe_extract = None;

            let mut exec_variants = Vec::new();

            let binding_type = match &t.binding {
                Some(ToolBinding::Exec(e)) => {
                    exec_command = if !e.command.is_empty() {
                        Some(e.command.clone())
                    } else {
                        None
                    };
                    exec_args = Some(e.args.clone());
                    exec_workdir = e.workdir.clone();
                    exec_timeout_ms = e.timeout_ms;
                    exec_envs = e.envs.clone();
                    capture_stdout = e.capture_stdout;
                    capture_stderr = e.capture_stderr;
                    for v in &e.variants {
                        exec_variants.push(CompiledExecVariant {
                            os: v.os.as_str().to_string(),
                            command: v.command.clone(),
                            args: v.args.clone(),
                            workdir: v.workdir.clone(),
                            timeout_ms: v.timeout_ms,
                            capture_stdout: v.capture_stdout,
                            capture_stderr: v.capture_stderr,
                            envs: v.envs.clone(),
                        });
                    }
                    "exec".to_string()
                }
                Some(ToolBinding::Http(h)) => {
                    http_method = Some(h.method.clone());
                    http_url = Some(h.url.clone());
                    http_headers = h.headers.clone();
                    http_query = h.query.clone();
                    http_body = h.body.clone();
                    http_timeout_ms = h.timeout_ms;
                    http_extract = h.extract_json.clone();
                    "http".to_string()
                }
                Some(ToolBinding::Ipc(ipc)) => {
                    ipc_dir = Some(ipc.dir.clone());
                    ipc_method = Some(ipc.method.clone());
                    ipc_params = ipc.params.clone();
                    ipc_timeout_ms = ipc.timeout_ms;
                    "ipc".to_string()
                }
                Some(ToolBinding::Native(n)) => {
                    exec_command = Some(n.symbol.clone());
                    "native".to_string()
                }
                Some(ToolBinding::Com(c)) => {
                    com_progid = Some(c.progid.clone());
                    com_method = Some(c.method.clone());
                    com_args = Some(c.args.clone());
                    com_attach = c.attach;
                    com_bring_to_front = c.bring_to_front;
                    com_timeout_ms = c.timeout_ms;
                    "com".to_string()
                }
                Some(ToolBinding::Ws(ws)) => {
                    ws_url = ws.url.clone();
                    ws_host = ws.host.clone();
                    ws_port = ws.port.clone();
                    ws_endpoint = ws.endpoint.clone();
                    ws_headers = ws.headers.clone();
                    ws_message = Some(ws.message.clone());
                    ws_timeout_ms = ws.timeout_ms;
                    ws_extract = ws.extract_json.clone();
                    "ws".to_string()
                }
                Some(ToolBinding::Pipe(pipe)) => {
                    pipe_host = pipe.host.clone();
                    pipe_port = pipe.port.clone();
                    pipe_addr = pipe.addr.clone();
                    pipe_message = Some(pipe.message.clone());
                    pipe_framing = Some(pipe.framing.clone());
                    pipe_timeout_ms = pipe.timeout_ms;
                    pipe_extract = pipe.extract_json.clone();
                    "pipe".to_string()
                }
                None => "none".to_string(),
            };

            let defaults: Vec<(String, String)> = t
                .params
                .iter()
                .filter_map(|p| {
                    p.default
                        .as_ref()
                        .map(|d| (p.name.clone(), serde_json::to_string(d).unwrap_or_default()))
                })
                .collect();

            CompiledToolSpec {
                name: t.name.clone(),
                description: t.description.clone(),
                binding_type,
                output_format: match t.output.format {
                    OutputFormat::Text => "text".to_string(),
                    OutputFormat::Image => "image".to_string(),
                    OutputFormat::Json => "json".to_string(),
                },
                mime_type: t
                    .output
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "text/plain".to_string()),
                output_trim: t.output.trim,
                output_slice_lines: t.output.slice.as_ref().map(|s| s.lines),
                output_slice_head: t.output.slice.as_ref().map(|s| s.head).unwrap_or(true),
                output_extract_json: t.output.extract_json.clone(),
                output_filter_not: t.output.filter_not.clone(),
                output_filter: t.output.filter.clone(),
                output_regex_pattern: t.output.regex.as_ref().map(|r| r.pattern.clone()),
                output_regex_template: t.output.regex.as_ref().and_then(|r| r.template.clone()),
                exec_command,
                exec_args,
                exec_workdir,
                exec_timeout_ms,
                exec_envs,
                exec_variants,
                capture_stdout,
                capture_stderr,
                http_method,
                http_url,
                http_headers,
                http_query,
                http_body,
                http_timeout_ms,
                http_extract,
                ipc_dir,
                ipc_method,
                ipc_params,
                ipc_timeout_ms,
                com_progid,
                com_method,
                com_args,
                com_attach,
                com_bring_to_front,
                com_timeout_ms,
                ws_url,
                ws_host,
                ws_port,
                ws_endpoint,
                ws_headers,
                ws_message,
                ws_timeout_ms,
                ws_extract,
                pipe_host,
                pipe_port,
                pipe_addr,
                pipe_message,
                pipe_framing,
                pipe_timeout_ms,
                pipe_extract,
                defaults,
            }
        })
        .collect()
}

pub fn compile_envs(envs: &[EnvVarSpec]) -> Vec<CompiledEnvSpec> {
    envs.iter()
        .map(|e| {
            let var_type = match e.var_type {
                EnvType::Path => "path",
                EnvType::Integer => "integer",
                EnvType::U16 => "u16",
                EnvType::Boolean => "boolean",
                EnvType::String => "string",
            }
            .to_string();

            let cli_long = if let Some(ref c) = e.cli {
                if c.starts_with("--") {
                    c.clone()
                } else if c.starts_with('-') {
                    format!("-{}", c)
                } else {
                    format!("--{}", c)
                }
            } else {
                let kebab = e.name.to_lowercase().replace('_', "-");
                format!("--{}", kebab)
            };

            let cli_short = e.short.as_ref().map(|s| {
                if s.starts_with('-') {
                    s.clone()
                } else {
                    format!("-{}", s)
                }
            });

            CompiledEnvSpec {
                name: e.name.clone(),
                var_type,
                required: e.required,
                default: e.default.clone().unwrap_or_default(),
                description: e.description.clone().unwrap_or_default(),
                cli_long,
                cli_short,
            }
        })
        .collect()
}
