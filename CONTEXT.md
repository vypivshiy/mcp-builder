# mcp-gen & std_mcp Domain Model

Zero-dependency and declarative tooling for Model Context Protocol (MCP) servers, including runtime libraries and the `mcp-builder` / `mcp-gen` KDL 2.0 IDL compiler.

## Crate Architecture & System Boundaries

The workspace is organized into two primary crates:
- **`mcp-builder-core`**: Core library containing the KDL 2.0 parser, AST data model, JSON Schema synthesizer, rich diagnostic reporting engine (`DiagnosticReport`), compile-time profile resolver (`ProfileResolver`), and Rust code generation engine (`CodeGenerator`).
- **`mcp-builder-cli`** (`mcp-builder` / `mcp-gen`): Command-line interface providing developer-facing commands: `check` (syntax & contract validation), `emit` (scaffold standalone Rust crate), and `build` (direct compilation to zero-dependency static binaries via `cargo build`).

## Domain Language & Glossary

**Server Node**:
The root declaration in an `mcp.kdl` document establishing server identity, version, and default transports.
_Avoid_: Config, RootBlock, AppHeader

**Tool**:
An executable capability exposed to LLM clients with a name, description, JSON Schema, and execution binding.
_Avoid_: Action, FunctionCall, Command

**Parameter Declaration**:
A typed input specification on a tool or resource template annotated with validation constraints.
_Avoid_: Arg, InputField, Prop

**Execution Binding**:
A declarative invocation block (`exec`, `http`, `ws`, `pipe`, `ipc`, `com`, `native`) connecting a tool or resource to an OS process, network endpoint, local IPC directory, or native Win32 COM automation server.
_Avoid_: ActionHandler, Runner, Target, Dispatcher

**Subprocess Binding (`bind:exec`)**:
A process invocation binding executing an external executable using `std::process::Command` without shell interpretation.
_Avoid_: ShellExec, ProcessRunner

**Conditional OS Variant (`when:<os>`)**:
A platform-specific branch inside an execution binding resolving at compile-time (`#[cfg(target_os = ...)]`) to specialize commands, arguments, or environment variables per OS.
_Avoid_: PlatformSwitch, OsBranch

**WebSocket Binding (`bind:ws`)**:
A network binding establishing a synchronous RFC 6455 client handshake and sending/receiving masked frames.
_Avoid_: SocketBinding, WsHandler

**Raw TCP Stream Binding (`bind:pipe`)**:
A network binding communicating with raw TCP sockets or bidirectional stream daemons using configurable delimiter framing.
_Avoid_: TcpStreamBinding, SocketPipe

**COM Automation Binding (`bind:com`)**:
A native Win32 OLE automation binding using dynamic `IDispatch` late-binding without external SDK dependencies.
_Avoid_: OleAutomation, ActiveXBridge

**Profile**:
A reusable, named wire-format template defining request wrapping, response mapping, parameter defaults, and framing for a transport or binding.
_Avoid_: Preset, Mixin, SchemaTemplate, ProtocolWrapper

**Output Mapping & Pipeline**:
A declarative transformation chain converting an execution binding's stdout, network payload, or COM result into standard MCP content blocks via line trimming, slicing (`head`/`tail`), filtering, regex captures, or JSON pointer extractions.
_Avoid_: ResponseFormatter, DataTransformer

**Resource**:
A readable URI endpoint providing static or dynamic text/binary context data to LLM clients.
_Avoid_: ContextProvider, DataEndpoint

**Resource Template**:
A parameterized URI pattern (RFC 6570 Level 1) resolving dynamic resources through execution bindings.
_Avoid_: DynamicResource, URIHandler, Route

**Prompt**:
A parameterized message template producing structured LLM conversation turns with role assignments (`system`, `user`, `assistant`).
_Avoid_: Template, PromptDefinition, Macro

**Environment Declaration**:
A top-level or server-level block declaring expected host environment variables with types, validation, defaults, and optional CLI flag mappings.
_Avoid_: ConfigVars, EnvBlock, SecretMap

**Template Parameter Modifier**:
A format modifier specified in placeholder substitutions (`{param:json}`, `{param:url}`, `{env:NAME:json}`, `{env:NAME:url}`) ensuring safe escaping and encoding of multiline strings, JSON payloads, or URL components.
_Avoid_: Filter, TemplateMacro, ValueTransformer

**Stdio Transport**:
Line-delimited JSON-RPC 2.0 communication over standard input and output with isolated stderr logging.
_Avoid_: CLI transport, Console mode

**SSE Transport**:
HTTP-based Server-Sent Events transport supporting bidirectional client-server interaction via GET `/sse` and POST `/message`.
_Avoid_: Webhook, HTTP polling

---

## Domain-to-AST & Runtime Mappings

| Domain Concept | Core AST Type (`mcp_builder_core::ast`) | Codegen / Runtime Type (`mcp_builder_core::codegen`) |
| :--- | :--- | :--- |
| **Server Node** | `ServerSpec`, `TransportSpec` | `CodeGenerator`, `templates::stdio_server`, `templates::sse_server` |
| **Tool** | `ToolSpec` | `CompiledToolSpec` |
| **Parameter Declaration** | `ParamSpec`, `ParamType` | JSON Schema Synthesizer (`schema.rs`), `CompiledParamSpec` |
| **Execution Binding** | `ToolBinding` | `CompiledToolSpec::binding_type` |
| **Subprocess Binding** | `ExecBinding` | `codegen::runtime::exec`, `std::process::Command` |
| **Conditional OS Variant** | `ExecVariant`, `TargetOs` | `CompiledExecVariant`, `#[cfg(target_os = ...)]` dispatch |
| **HTTP REST Binding** | `HttpBinding` | `codegen::runtime::http`, synchronous HTTP client |
| **WebSocket Binding** | `WsBinding` | `codegen::runtime::ws_pipe`, synchronous RFC 6455 client |
| **Raw TCP Stream Binding** | `PipeBinding` | `codegen::runtime::ws_pipe`, synchronous TCP stream framing |
| **COM Automation Binding** | `ComBinding` | `codegen::runtime::com`, Win32 `IDispatch` worker STA thread |
| **IPC Binding** | `IpcBinding` | `codegen::runtime::ipc`, filesystem IPC dispatch |
| **Native Binding** | `NativeBinding` | `codegen::runtime::native`, Rust direct symbol linkage |
| **Profile & Inheritance** | `ProfileSpec`, `ToolProfileRef` | `ProfileResolver`, built-in catalog (`cdp-chrome`, `redis-tcp`, etc.) |
| **Output Pipeline** | `OutputSpec`, `OutputFormat`, `OutputSliceSpec`, `OutputRegexSpec` | `codegen::runtime::output`, pipeline step executor |
| **Resource** | `ResourceSpec`, `ResourceContent` | `CompiledResourceSpec`, `resources/list` & `resources/read` router |
| **Resource Template** | `ResourceTemplateSpec` | `CompiledResourceTemplateSpec`, URI template parameter extractor |
| **Prompt** | `PromptSpec`, `PromptArgSpec`, `PromptMessageSpec` | `CompiledPromptSpec`, `prompts/list` & `prompts/get` router |
| **Environment Declaration** | `EnvVarSpec`, `EnvType` | `CompiledEnvSpec`, CLI flag parser & environment validator |
