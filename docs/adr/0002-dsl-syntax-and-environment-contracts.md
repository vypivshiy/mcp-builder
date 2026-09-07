# 0002: Declarative DSL Grammar, Environment Validation, and Output Mapping

We decided to structure the KDL 2.0 DSL around explicit node declarations (`server`, `env`, `tool`, `resource`, `resource-template`, `prompt`, `profile`) with strict separation between host environment variables and LLM input arguments.

To prevent secret leakage and ensure startup validation, host environment variables are declared in a top-level `env { ... }` block and referenced strictly via `{env:VAR}` inside execution bindings (`exec`, `http`, `ws`, `pipe`), keeping them completely isolated from LLM-facing tool schemas. Parameters declared via `param (type)"name"` are required by default, automatically synthesising strict JSON Schema Draft 7/2020-12 `inputSchema` objects. Output from subprocesses and network endpoints defaults to MCP text content blocks with automatic `isError: true` flag mapping on failures, while explicit `output format="image"` triggers automatic Base64 image block encoding.
