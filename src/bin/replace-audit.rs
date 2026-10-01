//! Temporary audit: literal `.replace` patterns that matched the old templates but not the new.
use std::path::{Path, PathBuf};

fn files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn corpus(roots: &[&str]) -> Vec<String> {
    let mut paths = Vec::new();
    for root in roots {
        files(Path::new(root), &mut paths);
    }
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

/// Decode the Rust string literal starting after its opening quote.
fn literal(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '\'' => out.push('\''),
                '\n' => {
                    while chars.clone().next().is_some_and(char::is_whitespace) {
                        chars.next();
                    }
                }
                _ => return None,
            },
            _ => out.push(c),
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let old = corpus(&[&args[1]]);
    let new = corpus(&[
        "game/mechs",
        "tests/fixtures/btech/mechs",
        "tests/fixtures/lua-probes/templates",
    ]);
    let mut sources = Vec::new();
    files(Path::new("src"), &mut sources);
    files(Path::new("tests"), &mut sources);
    for path in sources
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
    {
        let text = std::fs::read_to_string(path).unwrap();
        for (index, _) in text.match_indices(".replace") {
            let rest = &text[index..];
            let Some(open) = rest.find('(') else { continue };
            let after = rest[open + 1..].trim_start();
            let Some(body) = after.strip_prefix('"') else {
                continue;
            };
            let Some(pattern) = literal(body) else {
                continue;
            };
            if pattern.len() < 3 {
                continue;
            }
            let in_old = old.iter().any(|doc| doc.contains(&pattern));
            let in_new = new.iter().any(|doc| doc.contains(&pattern));
            if in_old && !in_new {
                let line = text[..index].matches('\n').count() + 1;
                println!("{}:{line}: {:?}", path.display(), pattern);
            }
        }
    }
}
