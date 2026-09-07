pub fn preprocess_kdl(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Handle strings: double-quoted and multiline
        if chars[i] == '"' {
            // Check for triple quotes
            if i + 2 < len && chars[i + 1] == '"' && chars[i + 2] == '"' {
                out.push_str("\"\"\"");
                i += 3;
                while i + 2 < len && !(chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"') {
                    if chars[i] == '\\' && i + 1 < len {
                        out.push(chars[i]);
                        out.push(chars[i + 1]);
                        i += 2;
                    } else {
                        out.push(chars[i]);
                        i += 1;
                    }
                }
                if i + 2 < len {
                    out.push_str("\"\"\"");
                    i += 3;
                }
                continue;
            } else {
                out.push('"');
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        out.push(chars[i]);
                        out.push(chars[i + 1]);
                        i += 2;
                    } else {
                        out.push(chars[i]);
                        i += 1;
                    }
                }
                if i < len {
                    out.push('"');
                    i += 1;
                }
                continue;
            }
        }

        // Handle raw string literals #"..."# or #"""..."""#
        if chars[i] == '#' && i + 1 < len && chars[i + 1] == '"' {
            out.push('r');
            out.push('#');
            out.push('"');
            i += 2;
            while i + 1 < len && !(chars[i] == '"' && chars[i + 1] == '#') {
                out.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                out.push('"');
                out.push('#');
                i += 2;
            }
            continue;
        }

        // Handle KDL 2.0 booleans and null: #true, #false, #null
        if chars[i] == '#' {
            let mut word = String::new();
            let mut j = i + 1;
            while j < len && chars[j].is_alphabetic() {
                word.push(chars[j]);
                j += 1;
            }
            match word.as_str() {
                "true" => {
                    out.push_str("true");
                    i = j;
                    continue;
                }
                "false" => {
                    out.push_str("false");
                    i = j;
                    continue;
                }
                "null" => {
                    out.push_str("null");
                    i = j;
                    continue;
                }
                _ => {
                    out.push('#');
                    i += 1;
                    continue;
                }
            }
        }

        // Handle (type)key=val -> key=(type)val
        if chars[i] == '(' {
            let mut j = i + 1;
            let mut ty = String::new();
            while j < len && chars[j] != ')' && chars[j] != '\n' {
                ty.push(chars[j]);
                j += 1;
            }
            if j < len && chars[j] == ')' {
                j += 1;
                // Now check if immediately followed by an identifier and '='
                let mut key = String::new();
                let mut k = j;
                while k < len && (chars[k].is_alphanumeric() || chars[k] == '-' || chars[k] == '_' || chars[k] == ':') {
                    key.push(chars[k]);
                    k += 1;
                }
                if !key.is_empty() && k < len && chars[k] == '=' {
                    // It was (type)key= -> rewrite as key=(type)
                    out.push_str(&key);
                    out.push('=');
                    out.push('(');
                    out.push_str(&ty);
                    out.push(')');
                    i = k + 1;
                    continue;
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_booleans() {
        let input = "enabled=#true disabled=#false val=#null";
        let out = preprocess_kdl(input);
        assert_eq!(out, "enabled=true disabled=false val=null");
    }

    #[test]
    fn test_preprocess_type_props() {
        let input = "param \"repo_path\" (string)type=\"string\" required=#true";
        let out = preprocess_kdl(input);
        assert_eq!(out, "param \"repo_path\" type=(string)\"string\" required=true");
    }
}
