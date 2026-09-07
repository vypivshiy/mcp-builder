# 0005: Template Parameter Modifiers for Safe JSON and URL Interpolation

We decided to introduce unified declarative placeholder modifiers (`{param:json}`, `{param:url}`, `{env:NAME:json}`, `{env:NAME:url}`) into the template interpolation engine.

When parameter or environment values containing quotes, unescaped backslashes, multiline text, or special URL characters are injected into raw template strings (such as JSON payloads in HTTP/WS bindings or URL query paths), direct textual substitution breaks data format validity. By introducing colon-suffixed modifiers with static compile-time diagnostic validation (`E0025`), templates can safely serialize complete valid JSON values (`:json` — auto-quoting/escaping multiline strings, objects, numbers, booleans, and arrays) and percent-encode query parameters (`:url`) without runtime crashes, manual quoting complexity, or external template engine bloat.
