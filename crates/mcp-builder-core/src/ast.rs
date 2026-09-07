use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub protocol_version: String,
    pub default_profile: Option<String>,
    pub transports: TransportSpec,
    pub envs: Vec<EnvVarSpec>,
    pub profiles: Vec<ProfileSpec>,
    pub tools: Vec<ToolSpec>,
    pub resources: Vec<ResourceSpec>,
    pub resource_templates: Vec<ResourceTemplateSpec>,
    pub prompts: Vec<PromptSpec>,
}

impl Default for ServerSpec {
    fn default() -> Self {
        Self {
            name: "mcp-server".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            license: None,
            protocol_version: "2024-11-05".to_string(),
            default_profile: None,
            transports: TransportSpec::default(),
            envs: Vec::new(),
            profiles: Vec::new(),
            tools: Vec::new(),
            resources: Vec::new(),
            resource_templates: Vec::new(),
            prompts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransportSpec {
    pub stdio: bool,
    pub sse: bool,
    pub sse_host: Option<String>,
    pub sse_port: Option<u16>,
    pub sse_endpoint: Option<String>,
    pub sse_message_endpoint: Option<String>,
    pub cors: bool,
}

impl Default for TransportSpec {
    fn default() -> Self {
        Self {
            stdio: true,
            sse: false,
            sse_host: Some("127.0.0.1".to_string()),
            sse_port: Some(8080),
            sse_endpoint: Some("/sse".to_string()),
            sse_message_endpoint: Some("/message".to_string()),
            cors: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnvType {
    #[default]
    String,
    Path,
    Integer,
    U16,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVarSpec {
    pub name: String,
    #[serde(default)]
    pub var_type: EnvType,
    pub required: bool,
    pub default: Option<String>,
    pub description: Option<String>,
    pub cli: Option<String>,
    pub short: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub profile: Option<ToolProfileRef>,
    pub params: Vec<ParamSpec>,
    pub binding: Option<ToolBinding>,
    pub output: OutputSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProfileSpec {
    pub name: String,
    pub description: Option<String>,
    pub extends: Option<String>,
    pub binding: Option<ToolBinding>,
    pub output: Option<OutputSpec>,
    pub method: Option<String>,
    pub url: Option<String>,
    pub host: Option<String>,
    pub port: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub progid: Option<String>,
    pub framing: Option<String>,
    pub message: Option<String>,
    pub headers: Vec<(String, String)>,
    pub params: Vec<(String, String)>,
    pub extract_json: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ToolProfileRef {
    pub name: String,
    pub method: Option<String>,
    pub url: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub host: Option<String>,
    pub port: Option<String>,
    pub message: Option<String>,
    pub framing: Option<String>,
    pub progid: Option<String>,
    pub params: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub extract_json: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub param_type: ParamType,
    pub description: Option<String>,
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub pattern: Option<String>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    String,
    Integer,
    Number,
    Boolean,
    Array(Box<ParamType>),
    Object,
}

impl ParamType {
    pub fn as_json_schema_type(&self) -> &'static str {
        match self {
            ParamType::String => "string",
            ParamType::Integer => "integer",
            ParamType::Number => "number",
            ParamType::Boolean => "boolean",
            ParamType::Array(_) => "array",
            ParamType::Object => "object",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolBinding {
    Exec(ExecBinding),
    Http(HttpBinding),
    Ipc(IpcBinding),
    Native(NativeBinding),
    Com(ComBinding),
    Ws(WsBinding),
    Pipe(PipeBinding),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsBinding {
    pub url: Option<String>,
    pub host: Option<String>,
    pub port: Option<String>,
    pub endpoint: Option<String>,
    pub headers: Vec<(String, String)>,
    pub message: String,
    pub timeout_ms: u64,
    pub extract_json: Option<String>,
}

impl Default for WsBinding {
    fn default() -> Self {
        Self {
            url: None,
            host: None,
            port: None,
            endpoint: None,
            headers: Vec::new(),
            message: String::new(),
            timeout_ms: 10_000,
            extract_json: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipeBinding {
    pub host: Option<String>,
    pub port: Option<String>,
    pub addr: Option<String>,
    pub message: String,
    pub framing: String,
    pub timeout_ms: u64,
    pub extract_json: Option<String>,
}

impl Default for PipeBinding {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            addr: None,
            message: String::new(),
            framing: "\n".to_string(),
            timeout_ms: 10_000,
            extract_json: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcBinding {
    pub dir: String,
    pub method: String,
    pub params: Option<String>,
    pub timeout_ms: u64,
}

impl Default for IpcBinding {
    fn default() -> Self {
        Self {
            dir: String::new(),
            method: String::new(),
            params: None,
            timeout_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetOs {
    Windows,
    Linux,
    Macos,
    Unix,
    Fallback,
}

impl Default for TargetOs {
    fn default() -> Self {
        TargetOs::Fallback
    }
}

impl TargetOs {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "windows" | "win" | "win32" => Some(TargetOs::Windows),
            "linux" => Some(TargetOs::Linux),
            "macos" | "darwin" | "osx" | "apple" => Some(TargetOs::Macos),
            "unix" | "posix" => Some(TargetOs::Unix),
            "fallback" | "default" | "any" | "all" | "*" => Some(TargetOs::Fallback),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TargetOs::Windows => "windows",
            TargetOs::Linux => "linux",
            TargetOs::Macos => "macos",
            TargetOs::Unix => "unix",
            TargetOs::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecVariant {
    pub os: TargetOs,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub capture_stdout: Option<bool>,
    #[serde(default)]
    pub capture_stderr: Option<bool>,
    #[serde(default)]
    pub envs: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecBinding {
    pub command: String,
    pub args: Vec<String>,
    pub workdir: Option<String>,
    pub timeout_ms: u64,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub envs: Vec<(String, String)>,
    #[serde(default)]
    pub variants: Vec<ExecVariant>,
}

impl Default for ExecBinding {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            workdir: None,
            timeout_ms: 10_000,
            capture_stdout: true,
            capture_stderr: true,
            envs: Vec::new(),
            variants: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpBinding {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout_ms: u64,
    pub extract_json: Option<String>,
}

impl Default for HttpBinding {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            url: String::new(),
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            timeout_ms: 10_000,
            extract_json: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeBinding {
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComBinding {
    pub progid: String,
    pub method: String,
    pub args: Vec<String>,
    pub attach: bool,
    pub bring_to_front: bool,
    pub timeout_ms: u64,
}

impl Default for ComBinding {
    fn default() -> Self {
        Self {
            progid: String::new(),
            method: String::new(),
            args: Vec::new(),
            attach: true,
            bring_to_front: false,
            timeout_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSpec {
    pub format: OutputFormat,
    pub mime_type: Option<String>,
    pub trim: bool,
    pub slice: Option<OutputSliceSpec>,
    pub extract_json: Option<String>,
    pub regex: Option<OutputRegexSpec>,
    pub filter_not: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSliceSpec {
    pub lines: usize,
    pub head: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputRegexSpec {
    pub pattern: String,
    pub template: Option<String>,
}

impl Default for OutputSpec {
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            mime_type: Some("text/plain".to_string()),
            trim: false,
            slice: None,
            extract_json: None,
            regex: None,
            filter_not: None,
            filter: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Image,
    Json,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub content: Option<ResourceContent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ResourceContent {
    Text { text: String },
    Binary { blob: String },
    Exec(ExecBinding),
    Native { symbol: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceTemplateSpec {
    pub uri_template: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    #[serde(default)]
    pub profile: Option<ToolProfileRef>,
    pub params: Vec<ParamSpec>,
    pub binding: Option<ToolBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSpec {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgSpec>,
    pub messages: Vec<PromptMessageSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptArgSpec {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptMessageSpec {
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
}
