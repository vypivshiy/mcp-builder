use kdl::{KdlNode, KdlValue};
use serde_json::Value as JsonValue;

/// Extracts a string property by key from a KDL node.
pub fn get_prop(node: &KdlNode, key: &str) -> Option<String> {
    for entry in node.entries() {
        if let Some(name) = entry.name() {
            if name.value() == key {
                return val_as_string(entry.value());
            }
        }
    }
    None
}

/// Extracts a boolean property by key from a KDL node.
pub fn get_prop_bool(node: &KdlNode, key: &str) -> Option<bool> {
    for entry in node.entries() {
        if let Some(name) = entry.name() {
            if name.value() == key {
                return val_as_bool(entry.value());
            }
        }
    }
    None
}

/// Extracts an unsigned 64-bit integer property by key from a KDL node.
pub fn get_prop_u64(node: &KdlNode, key: &str) -> Option<u64> {
    for entry in node.entries() {
        if let Some(name) = entry.name() {
            if name.value() == key {
                return val_as_u64(entry.value());
            }
        }
    }
    None
}

/// Extracts a 64-bit float property by key from a KDL node.
pub fn get_prop_f64(node: &KdlNode, key: &str) -> Option<f64> {
    for entry in node.entries() {
        if let Some(name) = entry.name() {
            if name.value() == key {
                return val_as_f64(entry.value());
            }
        }
    }
    None
}

/// Extracts a JSON-compatible value property by key from a KDL node.
pub fn get_prop_json(node: &KdlNode, key: &str) -> Option<JsonValue> {
    for entry in node.entries() {
        if let Some(name) = entry.name() {
            if name.value() == key {
                return Some(val_as_json(entry.value()));
            }
        }
    }
    None
}

/// Extracts a positional string argument by 0-indexed position from a KDL node.
pub fn get_arg_str(node: &KdlNode, idx: usize) -> Option<String> {
    let mut current = 0;
    for entry in node.entries() {
        if entry.name().is_none() {
            if current == idx {
                return val_as_string(entry.value());
            }
            current += 1;
        }
    }
    None
}

/// Extracts a positional boolean argument by 0-indexed position from a KDL node.
pub fn get_arg_bool(node: &KdlNode, idx: usize) -> Option<bool> {
    let mut current = 0;
    for entry in node.entries() {
        if entry.name().is_none() {
            if current == idx {
                return val_as_bool(entry.value());
            }
            current += 1;
        }
    }
    None
}

/// Extracts a positional unsigned 64-bit integer argument by 0-indexed position from a KDL node.
pub fn get_arg_u64(node: &KdlNode, idx: usize) -> Option<u64> {
    let mut current = 0;
    for entry in node.entries() {
        if entry.name().is_none() {
            if current == idx {
                return val_as_u64(entry.value());
            }
            current += 1;
        }
    }
    None
}

/// Extracts a positional 64-bit float argument by 0-indexed position from a KDL node.
pub fn get_arg_f64(node: &KdlNode, idx: usize) -> Option<f64> {
    let mut current = 0;
    for entry in node.entries() {
        if entry.name().is_none() {
            if current == idx {
                return val_as_f64(entry.value());
            }
            current += 1;
        }
    }
    None
}

/// Extracts a positional JSON-compatible value argument by 0-indexed position from a KDL node.
pub fn get_arg_json(node: &KdlNode, idx: usize) -> Option<JsonValue> {
    let mut current = 0;
    for entry in node.entries() {
        if entry.name().is_none() {
            if current == idx {
                return Some(val_as_json(entry.value()));
            }
            current += 1;
        }
    }
    None
}

/// Coerces a `KdlValue` into an owned `String`.
pub fn val_as_string(val: &KdlValue) -> Option<String> {
    match val {
        KdlValue::String(s) | KdlValue::RawString(s) => Some(s.clone()),
        KdlValue::Base10(i) | KdlValue::Base2(i) | KdlValue::Base8(i) | KdlValue::Base16(i) => Some(i.to_string()),
        KdlValue::Base10Float(f) => Some(f.to_string()),
        KdlValue::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Coerces a `KdlValue` into a `bool`.
pub fn val_as_bool(val: &KdlValue) -> Option<bool> {
    match val {
        KdlValue::Bool(b) => Some(*b),
        KdlValue::String(s) | KdlValue::RawString(s) => match s.to_lowercase().as_str() {
            "true" | "#true" | "1" | "yes" => Some(true),
            "false" | "#false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Coerces a `KdlValue` into an unsigned integer `u64`.
pub fn val_as_u64(val: &KdlValue) -> Option<u64> {
    match val {
        KdlValue::Base10(i) | KdlValue::Base2(i) | KdlValue::Base8(i) | KdlValue::Base16(i) if *i >= 0 => Some(*i as u64),
        KdlValue::Base10Float(f) if *f >= 0.0 => Some(*f as u64),
        KdlValue::String(s) | KdlValue::RawString(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

/// Coerces a `KdlValue` into a float `f64`.
pub fn val_as_f64(val: &KdlValue) -> Option<f64> {
    match val {
        KdlValue::Base10Float(f) => Some(*f),
        KdlValue::Base10(i) | KdlValue::Base2(i) | KdlValue::Base8(i) | KdlValue::Base16(i) => Some(*i as f64),
        KdlValue::String(s) | KdlValue::RawString(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Coerces a `KdlValue` into a `serde_json::Value`.
pub fn val_as_json(val: &KdlValue) -> JsonValue {
    match val {
        KdlValue::String(s) | KdlValue::RawString(s) => JsonValue::String(s.clone()),
        KdlValue::Base10(i) | KdlValue::Base2(i) | KdlValue::Base8(i) | KdlValue::Base16(i) => {
            JsonValue::Number(serde_json::Number::from(*i))
        }
        KdlValue::Base10Float(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        KdlValue::Bool(b) => JsonValue::Bool(*b),
        KdlValue::Null => JsonValue::Null,
    }
}

/// Allowed template parameter/environment placeholder modifiers.
pub const VALID_MODIFIERS: &[&str] = &["json", "url"];

pub fn is_valid_modifier(m: &str) -> bool {
    VALID_MODIFIERS.contains(&m)
}

/// Representation of a parsed placeholder token within a template string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderRef {
    pub raw: String,
    pub name: String,
    pub modifier: Option<String>,
    pub is_env: bool,
}

/// Parses all placeholder tokens (`{param}`, `{param:modifier}`, `{env:NAME}`, `{env:NAME:modifier}`, `$VAR`, `$param`) from template strings.
pub fn parse_placeholder_tokens(text: &str) -> Vec<PlaceholderRef> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len()
                && bytes[j] != b'}'
                && bytes[j] != b'{'
                && bytes[j] != b'"'
                && bytes[j] != b'\n'
                && bytes[j] != b'\r'
            {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'}' && j > start {
                let inner = &text[start..j];
                let raw = format!("{{{}}}", inner);

                if let Some(env_part) = inner.strip_prefix("env:") {
                    if !env_part.is_empty() {
                        let (name, modifier) = if let Some((n, m)) = env_part.split_once(':') {
                            (n.to_string(), if !m.is_empty() { Some(m.to_string()) } else { None })
                        } else {
                            (env_part.to_string(), None)
                        };
                        if !name.is_empty() {
                            tokens.push(PlaceholderRef {
                                raw,
                                name,
                                modifier,
                                is_env: true,
                            });
                        }
                    }
                } else if !inner.contains(' ') {
                    let (name, modifier) = if let Some((n, m)) = inner.split_once(':') {
                        (n.to_string(), if !m.is_empty() { Some(m.to_string()) } else { None })
                    } else {
                        (inner.to_string(), None)
                    };
                    if !name.is_empty() {
                        tokens.push(PlaceholderRef {
                            raw,
                            name,
                            modifier,
                            is_env: false,
                        });
                    }
                }
                i = j + 1;
                continue;
            }
        } else if bytes[i] == b'$' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j > start {
                let var_name = &text[start..j];
                let is_env = var_name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    && var_name.chars().any(|c| c.is_ascii_uppercase());
                tokens.push(PlaceholderRef {
                    raw: format!("${}", var_name),
                    name: var_name.to_string(),
                    modifier: None,
                    is_env,
                });
                i = j;
                continue;
            }
        }
        i += 1;
    }

    tokens
}

/// Extracts `{env:NAME}`, `{param_name}`, and `$ENV_VAR` / `$param` placeholders from template strings.
///
/// Returns a tuple `(envs, params)`.
pub fn extract_placeholders(text: &str) -> (Vec<String>, Vec<String>) {
    let tokens = parse_placeholder_tokens(text);
    let mut envs = Vec::new();
    let mut params = Vec::new();
    for t in tokens {
        if t.is_env {
            if !envs.contains(&t.name) {
                envs.push(t.name);
            }
        } else {
            if !params.contains(&t.name) {
                params.push(t.name);
            }
        }
    }
    (envs, params)
}

/// Validates regular expression syntax at compile time without regex dependencies.
pub fn validate_regex_pattern(pat: &str) -> Result<(), String> {
    let mut bracket_depth = 0;
    let mut paren_depth = 0;
    let mut chars = pat.chars().peekable();
    let mut is_first = true;

    while let Some(ch) = chars.next() {
        if is_first && (ch == '*' || ch == '+' || ch == '?') {
            return Err(format!("Dangling quantifier '{}' at start of pattern", ch));
        }
        is_first = false;

        match ch {
            '\\' => {
                if chars.next().is_none() {
                    return Err("Dangling escape '\\' at end of pattern".to_string());
                }
            }
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth == 0 {
                    return Err("Unmatched closing bracket ']'".to_string());
                }
                bracket_depth -= 1;
            }
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth == 0 {
                    return Err("Unmatched closing parenthesis ')'".to_string());
                }
                paren_depth -= 1;
            }
            _ => {}
        }
    }

    if bracket_depth > 0 {
        return Err("Unclosed character class '['".to_string());
    }
    if paren_depth > 0 {
        return Err("Unclosed parenthesis '('".to_string());
    }

    Ok(())
}

/// Calculates Levenshtein edit distance between two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, &ca) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }
        prev_row.copy_from_slice(&curr_row);
    }

    prev_row[b_len]
}

/// Finds the closest string match from a candidate set using Levenshtein distance.
pub fn find_closest_match<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let mut best_match = None;
    let mut best_dist = usize::MAX;

    for candidate in candidates {
        let dist = levenshtein_distance(target, candidate);
        let max_allowed = if target.len() <= 4 {
            1
        } else if target.len() <= 8 {
            2
        } else {
            3
        };
        if dist <= max_allowed && dist < best_dist {
            best_dist = dist;
            best_match = Some(candidate.to_string());
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_and_closest_match() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("rosettacode", "raisethysword"), 8);

        let candidates = vec!["exec", "http", "ws", "pipe", "com"];
        assert_eq!(find_closest_match("exe", candidates.iter().copied()), Some("exec".to_string()));
        assert_eq!(find_closest_match("htpp", candidates.iter().copied()), Some("http".to_string()));
        assert_eq!(find_closest_match("completely_unrelated", candidates.iter().copied()), None);
    }

    #[test]
    fn test_extract_placeholders() {
        let template = "curl -H 'Auth: {env:API_KEY}' -d '{user_id}' $BASE_URL $port";
        let (envs, params) = extract_placeholders(template);
        assert_eq!(envs, vec!["API_KEY".to_string(), "BASE_URL".to_string()]);
        assert_eq!(params, vec!["user_id".to_string(), "port".to_string()]);
    }

    #[test]
    fn test_parse_placeholder_tokens_with_modifiers() {
        let template = r#"{"code": {code:json}, "url": "{env:BASE:url}/{path:url}", "simple": "{name}"}"#;
        let tokens = parse_placeholder_tokens(template);
        assert_eq!(
            tokens,
            vec![
                PlaceholderRef {
                    raw: "{code:json}".to_string(),
                    name: "code".to_string(),
                    modifier: Some("json".to_string()),
                    is_env: false,
                },
                PlaceholderRef {
                    raw: "{env:BASE:url}".to_string(),
                    name: "BASE".to_string(),
                    modifier: Some("url".to_string()),
                    is_env: true,
                },
                PlaceholderRef {
                    raw: "{path:url}".to_string(),
                    name: "path".to_string(),
                    modifier: Some("url".to_string()),
                    is_env: false,
                },
                PlaceholderRef {
                    raw: "{name}".to_string(),
                    name: "name".to_string(),
                    modifier: None,
                    is_env: false,
                },
            ]
        );

        let (envs, params) = extract_placeholders(template);
        assert_eq!(envs, vec!["BASE".to_string()]);
        assert_eq!(params, vec!["code".to_string(), "path".to_string(), "name".to_string()]);
    }

    #[test]
    fn test_validate_regex_pattern() {
        assert!(validate_regex_pattern(r"^[a-zA-Z0-9_]+$").is_ok());
        assert!(validate_regex_pattern(r"(\d+)-(\w+)").is_ok());
        assert!(validate_regex_pattern(r"*invalid").is_err());
        assert!(validate_regex_pattern(r"[unclosed").is_err());
        assert!(validate_regex_pattern(r"(unclosed").is_err());
    }
}
