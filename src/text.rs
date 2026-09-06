use regex::Regex;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthStr;
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());
static MARKUP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(?:/|(?:color|fg|bg|send|prompt|url|link|bold|italic|underline)[^\]]*)\]")
        .unwrap()
});
pub fn plain(s: &str) -> String {
    ANSI.replace_all(s, "").into_owned()
}
pub fn markup(s: &str) -> String {
    MARKUP.replace_all(s, "").into_owned()
}
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(plain(s).as_str())
}
pub fn truncate(s: &str, max: usize) -> String {
    let plain = plain(s);
    let mut result = String::new();
    for ch in plain.chars() {
        let mut next = result.clone();
        next.push(ch);
        if width(&next) > max {
            break;
        }
        result = next;
    }
    result
}
pub fn style(s: &str, color: &str) -> String {
    let code = match color {
        "bright-white" => 97,
        "bright-yellow" => 93,
        "bright-green" => 92,
        "bright-red" => 91,
        "red" => 31,
        "green" => 32,
        "yellow" => 33,
        "blue" => 34,
        "cyan" => 36,
        "white" => 37,
        _ => 0,
    };
    format!("\x1b[{code}m{s}\x1b[0m")
}
