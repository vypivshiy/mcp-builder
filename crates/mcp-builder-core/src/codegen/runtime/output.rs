pub const OUTPUT_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Declarative Output Transformation & Regex Engine (Pure Rust, Zero Dependencies)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MicroRegexItem {
    AnchorStart,
    AnchorEnd,
    Literal(char),
    Any,
    CharClass {
        negated: bool,
        ranges: Vec<(char, char)>,
        chars: Vec<char>,
    },
    Group {
        index: usize,
        name: Option<String>,
        items: Vec<MicroRegexItem>,
    },
    Quantifier {
        min: usize,
        max: Option<usize>,
        item: Box<MicroRegexItem>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct MicroRegexMatch {
    pub full_match: String,
    pub positional_groups: Vec<String>,
    pub named_groups: HashMap<String, String>,
}

pub struct MicroRegex {
    pub items: Vec<MicroRegexItem>,
    pub has_start_anchor: bool,
    pub group_count: usize,
}

impl MicroRegex {
    pub fn parse(pattern: &str) -> Result<Self, String> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut index = 0;
        let mut group_counter = 0;
        let items = Self::parse_sequence(&chars, &mut index, &mut group_counter, false)?;
        let has_start_anchor = items.first().map_or(false, |it| matches!(it, MicroRegexItem::AnchorStart));
        Ok(Self {
            items,
            has_start_anchor,
            group_count: group_counter,
        })
    }

    fn parse_sequence(
        chars: &[char],
        idx: &mut usize,
        group_counter: &mut usize,
        inside_group: bool,
    ) -> Result<Vec<MicroRegexItem>, String> {
        let mut items: Vec<MicroRegexItem> = Vec::new();

        while *idx < chars.len() {
            let ch = chars[*idx];
            if inside_group && ch == ')' {
                break;
            }

            match ch {
                '^' if *idx == 0 || (inside_group && items.is_empty()) => {
                    items.push(MicroRegexItem::AnchorStart);
                    *idx += 1;
                }
                '$' if *idx + 1 == chars.len() || (inside_group && *idx + 1 < chars.len() && chars[*idx + 1] == ')') => {
                    items.push(MicroRegexItem::AnchorEnd);
                    *idx += 1;
                }
                '.' => {
                    items.push(MicroRegexItem::Any);
                    *idx += 1;
                }
                '\\' => {
                    *idx += 1;
                    if *idx >= chars.len() {
                        return Err("Dangling escape '\\'".to_string());
                    }
                    let esc = chars[*idx];
                    *idx += 1;
                    match esc {
                        'd' => items.push(MicroRegexItem::CharClass {
                            negated: false,
                            ranges: vec![('0', '9')],
                            chars: Vec::new(),
                        }),
                        'D' => items.push(MicroRegexItem::CharClass {
                            negated: true,
                            ranges: vec![('0', '9')],
                            chars: Vec::new(),
                        }),
                        'w' => items.push(MicroRegexItem::CharClass {
                            negated: false,
                            ranges: vec![('a', 'z'), ('A', 'Z'), ('0', '9')],
                            chars: vec!['_'],
                        }),
                        'W' => items.push(MicroRegexItem::CharClass {
                            negated: true,
                            ranges: vec![('a', 'z'), ('A', 'Z'), ('0', '9')],
                            chars: vec!['_'],
                        }),
                        's' => items.push(MicroRegexItem::CharClass {
                            negated: false,
                            ranges: Vec::new(),
                            chars: vec![' ', '\t', '\r', '\n'],
                        }),
                        'S' => items.push(MicroRegexItem::CharClass {
                            negated: true,
                            ranges: Vec::new(),
                            chars: vec![' ', '\t', '\r', '\n'],
                        }),
                        'n' => items.push(MicroRegexItem::Literal('\n')),
                        't' => items.push(MicroRegexItem::Literal('\t')),
                        'r' => items.push(MicroRegexItem::Literal('\r')),
                        other => items.push(MicroRegexItem::Literal(other)),
                    }
                }
                '[' => {
                    *idx += 1;
                    let mut negated = false;
                    if *idx < chars.len() && chars[*idx] == '^' {
                        negated = true;
                        *idx += 1;
                    }
                    let mut ranges = Vec::new();
                    let mut single_chars = Vec::new();
                    while *idx < chars.len() && chars[*idx] != ']' {
                        if chars[*idx] == '\\' && *idx + 1 < chars.len() {
                            *idx += 1;
                            let esc = chars[*idx];
                            *idx += 1;
                            match esc {
                                'd' => ranges.push(('0', '9')),
                                's' => single_chars.extend_from_slice(&[' ', '\t', '\r', '\n']),
                                'w' => {
                                    ranges.push(('a', 'z'));
                                    ranges.push(('A', 'Z'));
                                    ranges.push(('0', '9'));
                                    single_chars.push('_');
                                }
                                'n' => single_chars.push('\n'),
                                't' => single_chars.push('\t'),
                                'r' => single_chars.push('\r'),
                                o => single_chars.push(o),
                            }
                        } else if *idx + 2 < chars.len() && chars[*idx + 1] == '-' && chars[*idx + 2] != ']' {
                            let start_c = chars[*idx];
                            let end_c = chars[*idx + 2];
                            ranges.push((start_c, end_c));
                            *idx += 3;
                        } else {
                            single_chars.push(chars[*idx]);
                            *idx += 1;
                        }
                    }
                    if *idx < chars.len() && chars[*idx] == ']' {
                        *idx += 1;
                    }
                    items.push(MicroRegexItem::CharClass {
                        negated,
                        ranges,
                        chars: single_chars,
                    });
                }
                '(' => {
                    *idx += 1;
                    *group_counter += 1;
                    let group_idx = *group_counter;
                    let mut group_name = None;
                    if *idx + 2 < chars.len() && chars[*idx] == '?' && (chars[*idx + 1] == 'P' || chars[*idx + 1] == '<') {
                        if chars[*idx + 1] == 'P' && *idx + 3 < chars.len() && chars[*idx + 2] == '<' {
                            *idx += 3;
                            let mut name = String::new();
                            while *idx < chars.len() && chars[*idx] != '>' {
                                name.push(chars[*idx]);
                                *idx += 1;
                            }
                            if *idx < chars.len() && chars[*idx] == '>' {
                                *idx += 1;
                            }
                            group_name = Some(name);
                        } else if chars[*idx + 1] == '<' {
                            *idx += 2;
                            let mut name = String::new();
                            while *idx < chars.len() && chars[*idx] != '>' {
                                name.push(chars[*idx]);
                                *idx += 1;
                            }
                            if *idx < chars.len() && chars[*idx] == '>' {
                                *idx += 1;
                            }
                            group_name = Some(name);
                        }
                    }
                    let inner_items = Self::parse_sequence(chars, idx, group_counter, true)?;
                    if *idx < chars.len() && chars[*idx] == ')' {
                        *idx += 1;
                    }
                    items.push(MicroRegexItem::Group {
                        index: group_idx,
                        name: group_name,
                        items: inner_items,
                    });
                }
                '*' | '+' | '?' | '{' => {
                    if let Some(prev) = items.pop() {
                        let (min, max) = match ch {
                            '*' => {
                                *idx += 1;
                                (0, None)
                            }
                            '+' => {
                                *idx += 1;
                                (1, None)
                            }
                            '?' => {
                                *idx += 1;
                                (0, Some(1))
                            }
                            '{' => {
                                *idx += 1;
                                let mut num_str = String::new();
                                while *idx < chars.len() && chars[*idx] != '}' {
                                    num_str.push(chars[*idx]);
                                    *idx += 1;
                                }
                                if *idx < chars.len() && chars[*idx] == '}' {
                                    *idx += 1;
                                }
                                if let Some((min_s, max_s)) = num_str.split_once(',') {
                                    let min_val = min_s.trim().parse::<usize>().unwrap_or(0);
                                    let max_val = if max_s.trim().is_empty() {
                                        None
                                    } else {
                                        max_s.trim().parse::<usize>().ok()
                                    };
                                    (min_val, max_val)
                                } else {
                                    let n = num_str.trim().parse::<usize>().unwrap_or(1);
                                    (n, Some(n))
                                }
                            }
                            _ => (1, Some(1)),
                        };
                        items.push(MicroRegexItem::Quantifier {
                            min,
                            max,
                            item: Box::new(prev),
                        });
                    } else {
                        *idx += 1;
                    }
                }
                c => {
                    items.push(MicroRegexItem::Literal(c));
                    *idx += 1;
                }
            }
        }

        Ok(items)
    }

    pub fn matches(&self, text: &str) -> bool {
        self.find_match(text).is_some()
    }

    pub fn find_match(&self, text: &str) -> Option<MicroRegexMatch> {
        let input: Vec<char> = text.chars().collect();

        if self.has_start_anchor {
            let mut captures = HashMap::new();
            let mut named_map = HashMap::new();
            if let Some(end_pos) = Self::match_items(&self.items, &input, 0, &mut captures, &mut named_map) {
                let full_slice = &input[0..end_pos];
                let full_str: String = full_slice.iter().collect();
                return Some(Self::build_match(&input, full_str, captures, named_map, self.group_count));
            }
            None
        } else {
            for start_idx in 0..=input.len() {
                let mut captures = HashMap::new();
                let mut named_map = HashMap::new();
                if let Some(end_pos) = Self::match_items(&self.items, &input, start_idx, &mut captures, &mut named_map) {
                    let full_slice = &input[start_idx..end_pos];
                    let full_str: String = full_slice.iter().collect();
                    return Some(Self::build_match(&input, full_str, captures, named_map, self.group_count));
                }
            }
            None
        }
    }

    fn build_match(
        input: &[char],
        full_str: String,
        captures: HashMap<usize, (usize, usize)>,
        named_map: HashMap<String, usize>,
        group_count: usize,
    ) -> MicroRegexMatch {
        let mut positional_groups = Vec::new();
        for g_idx in 1..=group_count {
            if let Some(&(s, e)) = captures.get(&g_idx) {
                let g_str: String = input[s..e].iter().collect();
                positional_groups.push(g_str);
            } else {
                positional_groups.push(String::new());
            }
        }

        let mut named_groups = HashMap::new();
        for (name, g_idx) in named_map {
            if let Some(&(s, e)) = captures.get(&g_idx) {
                let g_str: String = input[s..e].iter().collect();
                named_groups.insert(name, g_str);
            }
        }

        MicroRegexMatch {
            full_match: full_str,
            positional_groups,
            named_groups,
        }
    }

    fn match_items(
        items: &[MicroRegexItem],
        input: &[char],
        pos: usize,
        captures: &mut HashMap<usize, (usize, usize)>,
        named_map: &mut HashMap<String, usize>,
    ) -> Option<usize> {
        if items.is_empty() {
            return Some(pos);
        }

        match &items[0] {
            MicroRegexItem::AnchorStart => {
                if pos == 0 {
                    Self::match_items(&items[1..], input, pos, captures, named_map)
                } else {
                    None
                }
            }
            MicroRegexItem::AnchorEnd => {
                if pos == input.len()
                    || (pos + 1 == input.len() && (input[pos] == '\n' || input[pos] == '\r'))
                    || (pos + 2 == input.len() && input[pos] == '\r' && input[pos + 1] == '\n')
                {
                    Self::match_items(&items[1..], input, pos, captures, named_map)
                } else {
                    None
                }
            }
            MicroRegexItem::Literal(c) => {
                if pos < input.len() && input[pos] == *c {
                    Self::match_items(&items[1..], input, pos + 1, captures, named_map)
                } else {
                    None
                }
            }
            MicroRegexItem::Any => {
                if pos < input.len() {
                    Self::match_items(&items[1..], input, pos + 1, captures, named_map)
                } else {
                    None
                }
            }
            MicroRegexItem::CharClass { negated, ranges, chars } => {
                if pos < input.len() {
                    let ch = input[pos];
                    let in_set = chars.contains(&ch) || ranges.iter().any(|(s, e)| ch >= *s && ch <= *e);
                    let matched = if *negated { !in_set } else { in_set };
                    if matched {
                        Self::match_items(&items[1..], input, pos + 1, captures, named_map)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            MicroRegexItem::Group { index, name, items: group_items } => {
                let prev_cap = captures.get(index).copied();
                if let Some(name_str) = name {
                    named_map.insert(name_str.clone(), *index);
                }
                if let Some(next_pos) = Self::match_items(group_items, input, pos, captures, named_map) {
                    captures.insert(*index, (pos, next_pos));
                    if let Some(res) = Self::match_items(&items[1..], input, next_pos, captures, named_map) {
                        return Some(res);
                    }
                }
                if let Some(pc) = prev_cap {
                    captures.insert(*index, pc);
                } else {
                    captures.remove(index);
                }
                None
            }
            MicroRegexItem::Quantifier { min, max, item } => {
                let mut match_positions = vec![pos];
                let mut current_pos = pos;
                let mut step_count = 0;

                while max.map_or(true, |m| step_count < m) {
                    let mut dummy_caps = captures.clone();
                    let mut dummy_named = named_map.clone();
                    if let Some(next_p) = Self::match_single_item(item, input, current_pos, &mut dummy_caps, &mut dummy_named) {
                        if next_p == current_pos {
                            break;
                        }
                        current_pos = next_p;
                        step_count += 1;
                        match_positions.push(current_pos);
                    } else {
                        break;
                    }
                }

                let start_idx = if match_positions.len() > *min { match_positions.len() - 1 } else { 0 };
                for count in ( *min..=start_idx ).rev() {
                    let target_pos = match_positions[count];
                    if let Some(res) = Self::match_items(&items[1..], input, target_pos, captures, named_map) {
                        return Some(res);
                    }
                }
                None
            }
        }
    }

    fn match_single_item(
        item: &MicroRegexItem,
        input: &[char],
        pos: usize,
        captures: &mut HashMap<usize, (usize, usize)>,
        named_map: &mut HashMap<String, usize>,
    ) -> Option<usize> {
        Self::match_items(std::slice::from_ref(item), input, pos, captures, named_map)
    }
}

pub fn apply_output_pipeline(
    raw_text: String,
    extract_json: Option<&str>,
    filter_not: Option<&str>,
    filter: Option<&str>,
    slice: Option<(usize, bool)>,
    regex_spec: Option<(&str, Option<&str>)>,
    trim: bool,
) -> Result<String, String> {
    let mut current = raw_text;

    // 1. Extract JSON Pointer / Path if configured
    if let Some(pointer_or_path) = extract_json {
        let json_val: Value = serde_json::from_str(current.trim())
            .map_err(|e| format!("Failed to parse tool output as JSON for extraction: {}", e))?;

        let extracted = if pointer_or_path.starts_with('/') {
            json_val.pointer(pointer_or_path)
        } else {
            let mut ptr = &json_val;
            for segment in pointer_or_path.split('.').filter(|s| !s.is_empty()) {
                if let Some(obj) = ptr.as_object() {
                    if let Some(next) = obj.get(segment) {
                        ptr = next;
                    } else {
                        ptr = &Value::Null;
                        break;
                    }
                } else if let Some(arr) = ptr.as_array() {
                    if let Ok(idx) = segment.parse::<usize>() {
                        if let Some(next) = arr.get(idx) {
                            ptr = next;
                        } else {
                            ptr = &Value::Null;
                            break;
                        }
                    } else {
                        ptr = &Value::Null;
                        break;
                    }
                } else {
                    ptr = &Value::Null;
                    break;
                }
            }
            if ptr.is_null() && !json_val.is_null() {
                None
            } else {
                Some(ptr)
            }
        };

        match extracted {
            Some(Value::String(s)) => current = s.clone(),
            Some(Value::Null) => current = "null".to_string(),
            Some(v) => current = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
            None => return Err(format!("JSON path '{}' not found in tool output", pointer_or_path)),
        }
    }

    // 2. Line Filtering (filter-not and filter)
    if filter_not.is_some() || filter.is_some() {
        let not_regex = filter_not.and_then(|pat| MicroRegex::parse(pat).ok());
        let keep_regex = filter.and_then(|pat| MicroRegex::parse(pat).ok());

        let lines: Vec<&str> = current.lines().collect();
        let mut filtered_lines = Vec::new();

        for line in lines {
            let mut drop = false;
            if let Some(ref r) = not_regex {
                if r.matches(line) {
                    drop = true;
                }
            } else if let Some(pat) = filter_not {
                if line.contains(pat) {
                    drop = true;
                }
            }

            if !drop {
                if let Some(ref r) = keep_regex {
                    if !r.matches(line) {
                        drop = true;
                    }
                } else if let Some(pat) = filter {
                    if !line.contains(pat) {
                        drop = true;
                    }
                }
            }

            if !drop {
                filtered_lines.push(line);
            }
        }

        current = filtered_lines.join("\n");
    }

    // 3. Line Slicing
    if let Some((lines_count, is_head)) = slice {
        let all_lines: Vec<&str> = current.lines().collect();
        if is_head {
            let sliced: Vec<&str> = all_lines.into_iter().take(lines_count).collect();
            current = sliced.join("\n");
        } else {
            let total = all_lines.len();
            if total > lines_count {
                let sliced = &all_lines[total - lines_count..];
                current = sliced.join("\n");
            }
        }
    }

    // 4. Regex Extraction & Template Interpolation
    if let Some((pattern, template_opt)) = regex_spec {
        let regex = MicroRegex::parse(pattern)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?;

        if let Some(mat) = regex.find_match(&current) {
            if let Some(tmpl) = template_opt {
                let mut rendered = tmpl.to_string();
                for (name, val) in &mat.named_groups {
                    let mut ph = String::from("{");
                    ph.push_str(name);
                    ph.push('}');
                    rendered = rendered.replace(&ph, val);
                }
                rendered = rendered.replace("{0}", &mat.full_match);
                for (idx, val) in mat.positional_groups.iter().enumerate() {
                    let mut ph = String::from("{");
                    ph.push_str(&(idx + 1).to_string());
                    ph.push('}');
                    rendered = rendered.replace(&ph, val);
                }
                current = rendered;
            } else if !mat.positional_groups.is_empty() && !mat.positional_groups[0].is_empty() {
                current = mat.positional_groups[0].clone();
            } else {
                current = mat.full_match;
            }
        } else {
            return Err(format!("Output did not match required pattern '{}'", pattern));
        }
    }

    // 5. Trimming
    if trim {
        current = current.trim().to_string();
    }

    Ok(current)
}
"###;
