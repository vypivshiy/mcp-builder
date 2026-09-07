# 0001: Declarative KDL 2.0 IDL as Application-Agnostic FastMCP Alternative

We decided to use KDL 2.0 as a purely declarative Interface Definition Language (IDL) for compiling native, zero-overhead Model Context Protocol (MCP) server binaries without any internal knowledge of target applications.

Instead of writing imperative Python or Node.js code with heavy runtime dependencies (Pydantic, Starlette, AnyIO) that consume 50–100 MB RAM and 400 ms boot times, developers declare tools, JSON schemas, resources, and execution bindings (`exec`, `http`, `ws`, `pipe`) in KDL 2.0. The compiler produces standalone, zero-dependency native binaries (<2 MB RAM, <1 ms cold start). Target-specific protocols (such as Chrome CDP or Blender Lab TCP) are expressed as purely textual, decoupled Profile templates, keeping the compiler core 100% application-agnostic.
