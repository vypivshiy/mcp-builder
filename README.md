# mcp-builder (`mcp-gen`)

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-blue.svg)](https://modelcontextprotocol.io)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

A high-performance declarative compiler for the [Model Context Protocol (MCP)](https://modelcontextprotocol.io).

`mcp-builder` compiles human-readable `mcp.kdl` specification files into standalone, zero-dependency native static binary MCP servers written in pure Rust. The resulting executables provide sub-millisecond cold starts and sub-2MB RAM footprints.

---

## Prerequisites

To build and compile MCP servers with `mcp-builder`, you need the **Rust toolchain** installed on your system:


>  `mcp-builder build` synthesizes typed Rust code from your `.kdl` schema and invokes `cargo build` in the background to produce a self-contained, high-performance native binary without external runtime dependencies (no Node.js, Python, or JVM required at runtime).

---

## Quickstart (Under 3 Minutes)


### 1. Install `mcp-builder`

Directly from GitHub (without cloning):
```sh
cargo install --git https://github.com/vypivshiy/mcp-builder mcp-builder-cli
```

Or from a locally cloned repository:
```sh
cargo install --path crates/mcp-builder-cli
```
This installs the `mcp-builder` CLI (and its alias `mcp-gen`).

---

### 2. Create your `mcp.kdl` specification

Create a file named `my_tools.kdl`:

```kdl
server name="my-tools" version="1.0.0" {
    description "A minimal MCP server with system utilities"
}

transports {
    stdio enabled=#true
}

tool "eval_math" description="Evaluate a mathematical expression in Python" {
    (string)param "expr" required=#true description="Mathematical expression e.g. 2**10 or math.sqrt(144)"

    output format="text" {
        trim #true
    }

    bind:exec "python" {
        args "-c" "import math; print(eval('{expr}'))"
        timeout-ms 5000
    }
}
```

---

### 3. Check and Compile

```sh
# Step A: Validate syntax and types
mcp-builder check my_tools.kdl

# Step B: Compile into a standalone native executable
mcp-builder build my_tools.kdl -o my_tools_server.exe --release
```

---

### 4. Connect to an MCP Client (Claude, Cursor, OpenCode)

Add the compiled binary to your MCP client configuration (e.g., `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "my-tools": {
      "command": "C:\\path\\to\\my_tools_server.exe"
    }
  }
}
```

Or run it as a standalone HTTP/SSE server:
```sh
./my_tools_server.exe --transport sse --port 8080
```

---

## AI & LLM-Powered Authoring

Because `mcp.kdl` is completely declarative and human-readable, **LLMs (ChatGPT, Claude, Cursor, Copilot, OpenCode) can author these files directly**.

You don't need to write repetitive boilerplate code. You can prompt an AI agent:

> *"Generate an `mcp.kdl` file for interacting with the GitHub REST API to list repositories and create issues with auth tokens from `env`."*

The LLM outputs the `.kdl` schema, and you can compile it into a production-ready binary:
```sh
mcp-builder build github_mcp.kdl -o dist/github_mcp.exe --release
```

---

## API & DSL Reference

### 1. Server Metadata (`server`)
```kdl
server name="api-server" version="1.0.0" {
    description "Production MCP Server"
    author "Developer"
    license "MIT"
    protocol-version "2024-11-05"
}
```

### 2. Transports (`transports`)
Configure Stdio and/or HTTP Server-Sent Events (SSE):
```kdl
transports {
    stdio enabled=#true
    sse enabled=#true {
        host "127.0.0.1"
        port 8080
        endpoint "/sse"
        message-endpoint "/message"
        cors #true
    }
}
```

### 3. Isolated Host Environment (`env`)
Declares host environment variables needed by bindings (e.g. API keys, directories). Environment variables are strictly isolated from the LLM tool schemas:
```kdl
env {
    (path)PROJECT_DIR default="." cli="--project-dir" short="-p" description="Working directory"
    (string)API_KEY required=#true cli="--api-key" description="Bearer auth token"
    (u16)PORT default=9000 description="Internal service port"
}
```

### 4. Tools (`tool`) & Parameter Types
Define capabilities exposed to LLMs with strict schema validation:
```kdl
tool "search" description="Search database records" {
    (string)param "query" required=#true description="Search query string"
    (integer)param "limit" default=10 minimum=1 maximum=100 description="Max results"
    (string)param "status" default="active" description="Record status" {
        enum "active" "pending" "archived"
    }

    output format="text" {
        trim #true
        head 50
    }

    bind:exec "search-cli" {
        args "--query" "{query}" "--limit" "{limit}" "--status" "{status}"
        workdir "{env:PROJECT_DIR}"
    }
}
```

Supported parameter types: `(string)`, `(integer)`, `(number)`, `(boolean)`, `(array)`, `(object)`.

### 5. Template Modifiers (Safe Escaping)
- `{param}` / `{env:VAR}`: Plain text substitution.
- `{param:json}` / `{env:VAR:json}`: Safe JSON serialization (automatically handles string quoting, multiline escaping, objects, arrays, booleans, and null).
- `{param:url}` / `{env:VAR:url}`: RFC 3986 percent-encoding for URL queries and paths.

### 6. Output Processing Pipelines (`output`)
Post-process stdout or network responses before sending them back to the LLM:
```kdl
output format="text" {
    trim #true                      // Strip whitespace
    head 25                         // Keep first 25 lines
    tail 10                         // Keep last 10 lines
    filter "ERROR"                  // Keep only lines matching substring
    filter-not "DEBUG"              // Remove lines matching substring
    extract-json "/result/items"    // Extract field via RFC 6901 JSON pointer
}
```

### 7. Execution Bindings

#### A. CLI Subprocess (`bind:exec`) with Cross-Platform Dispatch
```kdl
tool "list_files" description="List directory contents" {
    bind:exec {
        when:windows "powershell" {
            args "-NoProfile" "-Command" "Get-ChildItem -Name"
        }
        when:unix "ls" {
            args "-la"
        }
    }
}
```

#### B. REST / HTTP API (`bind:http`)
```kdl
tool "get_weather" description="Fetch current weather report" {
    (string)param "city" required=#true description="City name"

    bind:http method="GET" url="https://api.weather.com/v1/current?city={city:url}" {
        header "Authorization" "Bearer {env:API_KEY}"
        header "Accept" "application/json"
        extract-json "/data/temperature"
    }
}
```

#### C. WebSocket RPC (`bind:ws`)
```kdl
tool "cdp_navigate" description="Navigate headless Chrome tab" {
    (string)param "url" required=#true description="Target URL"

    bind:ws url="ws://127.0.0.1:9222/devtools/page/ACTIVE_TAB" {
        message #"{"id": 1, "method": "Page.navigate", "params": {"url": {url:json}}}"#
        extract-json "/result/frameId"
    }
}
```

#### D. Raw TCP Socket Stream (`bind:pipe`)
```kdl
tool "redis_ping" description="Ping Redis daemon" {
    bind:pipe host="127.0.0.1" port="6379" {
        framing "\r\n"
        message "PING\r\n"
    }
}
```

#### E. Native Windows COM Automation (`bind:com`)
```kdl
tool "speak" description="Text-to-speech via Windows SAPI" {
    (string)param "text" required=#true description="Text to speak aloud"

    bind:com progid="SAPI.SpVoice" dispatch="Speak" {
        args "{text}"
    }
}
```

### 8. Static & Dynamic Resources (`resource`, `resource-template`)
```kdl
// Static embedded resource
resource "docs://quickstart" name="Quickstart Guide" {
    description "Embedded developer documentation"
    mime-type "text/markdown"
    text """
    # Quickstart Guide
    Welcome to the embedded server docs.
    """
}

// Dynamic parameterized resource template (RFC 6570)
resource-template "logs://services/{service_id}" name="Service Logs" {
    description "Tail logs for a given service"
    mime-type "text/plain"
    (string)param "service_id" required=#true description="Service identifier"

    bind:exec "journalctl" {
        args "-u" "{service_id}" "-n" "50" "--no-pager"
    }
}
```

### 9. Reusable Prompts (`prompt`)
```kdl
prompt "code_review" description="Structured code review workflow" {
    argument "code" required=#true description="Source code snippet"

    message role="system" "You are a senior Rust engineer reviewing code for memory safety and performance."
    message role="user" "Review this code:\n\n```rust\n{code}\n```"
}
```

---

## CLI Commands Reference

| Command | Usage | Description |
|---|---|---|
| `mcp-builder check <file.kdl>` | `mcp-builder check mcp.kdl` | Lints schema, checks types, verifies template variables and binding definitions. |
| `mcp-builder emit <file.kdl> -o <dir>` | `mcp-builder emit mcp.kdl -o ./src` | Emits the fully generated Rust project source for manual inspection. |
| `mcp-builder build <file.kdl> -o <exe>` | `mcp-builder build mcp.kdl -o server.exe --release` | Compiles a standalone native executable ready for deployment. |

---

## Ready-to-Use Examples

Explore complete production configurations in the [`examples/`](examples/) directory:

- [`crossplatform_system_cli.kdl`](examples/crossplatform_system_cli.kdl) - Cross-platform CLI utilities (Windows PowerShell / Unix).
- [`chrome_cdp.kdl`](examples/chrome_cdp.kdl) - Chrome DevTools Protocol browser automation via WebSocket.
- [`obs_control.kdl`](examples/obs_control.kdl) - OBS Studio v5 streaming and recording controller.
- [`httpbin.kdl`](examples/httpbin.kdl) - REST API client testing suite with JSON extraction.
- [`windows_native_com.kdl`](examples/windows_native_com.kdl) - Windows 11 COM Automation (`SAPI.SpVoice` and `WScript.Shell`).
- [`python_debugger_pipe.kdl`](examples/python_debugger_pipe.kdl) - Raw TCP stream bridge to remote Python debug runtimes.
- [`network_bindings.kdl`](examples/network_bindings.kdl) - Multi-protocol bridge (Chrome CDP + Redis TCP).
- [`ida_mcp.kdl`](examples/ida_mcp.kdl) - Binary analysis & decompilation bridge for IDA Pro.

---

## Running Tests

```sh
# Run compiler unit tests
cargo test -p mcp-builder-core

# Run end-to-end integration tests (includes mock HTTP, WS, TCP, and CLI daemons)
cargo test --test test_cli -- --test-threads=1
```

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
