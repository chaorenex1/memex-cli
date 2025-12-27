pub(crate) fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn truncate_clean(s: &str, max_chars: usize) -> String {
    let mut t = s.trim().to_string();
    t = t.replace("\r\n", "\n");
    if t.chars().count() <= max_chars {
        return t;
    }
    let mut out = String::new();
    for (i, ch) in t.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str(" ...");
    out
}

pub(crate) fn trim_mid(s: &str, max_chars: usize) -> String {
    let t = one_line(s);
    if t.chars().count() <= max_chars {
        return t;
    }
    let head: String = t.chars().take(max_chars.saturating_sub(2)).collect();
    format!("{head}..")
}
