//! Render and check LuaLS declarations maintained beside the Rust Lua bindings.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// A numbered LuaLS block and the Rust source location that owns it.
struct Contract {
    text: String,
    source: PathBuf,
    line: usize,
}

/// CLI action; exactly one action is required.
enum Mode {
    Check,
    Write,
}

/// Repository checkout that holds this crate at `crates/lua-tools`.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/lua-tools sits two levels below the repository root")
        .to_path_buf()
}

/// Parse the deliberately small command-line interface.
fn arguments() -> Result<(Mode, PathBuf)> {
    let mut mode = None;
    let mut root = repository_root();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" if mode.is_none() => mode = Some(Mode::Check),
            "--write" if mode.is_none() => mode = Some(Mode::Write),
            "--repo-root" => {
                root = PathBuf::from(args.next().context("--repo-root needs a path")?);
            }
            _ => bail!("unknown or repeated option: {arg}"),
        }
    }
    Ok((
        mode.context("specify exactly one of --check or --write")?,
        root,
    ))
}

/// Visit only Rust binding sources, in a stable order.
fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            rust_sources(&path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
    sources.sort();
    Ok(())
}

/// Read explicitly numbered contract comments without interpreting Rust syntax.
fn contracts(root: &Path) -> Result<BTreeMap<String, BTreeMap<usize, Contract>>> {
    let mut sources = Vec::new();
    rust_sources(&root.join("src/lua/packages"), &mut sources)?;
    let mut modules: BTreeMap<String, BTreeMap<usize, Contract>> = BTreeMap::new();
    let mut callables = BTreeSet::new();
    let mut definitions = BTreeSet::new();
    for source in sources {
        let input = fs::read_to_string(&source)?;
        let mut active: Option<(String, usize, usize, Vec<String>)> = None;
        for (offset, line) in input.lines().enumerate() {
            let location = format!("{}:{}", source.display(), offset + 1);
            if let Some(marker) = line.strip_prefix("// lua-types-begin ") {
                if active.is_some() {
                    bail!("{location}: nested Lua type contract");
                }
                let (module, index) = marker.split_once(' ').context("invalid contract marker")?;
                if module != "mux" && module != "btech" {
                    bail!("{location}: unknown Lua module {module}");
                }
                let index = index
                    .parse::<usize>()
                    .with_context(|| format!("{location}: invalid contract index"))?;
                active = Some((module.to_owned(), index, offset + 1, Vec::new()));
            } else if line == "// lua-types-end" {
                let (module, index, start, payload) = active
                    .take()
                    .with_context(|| format!("{location}: unmatched contract end"))?;
                if payload.is_empty() {
                    bail!("{}:{start}: empty Lua type contract", source.display());
                }
                let text = payload.join("\n");
                for line in text.lines() {
                    for kind in ["class", "alias"] {
                        if let Some(declaration) = line.strip_prefix(&format!("---@{kind} ")) {
                            let declaration =
                                declaration.strip_prefix("(exact) ").unwrap_or(declaration);
                            let name = declaration
                                .split(|character: char| {
                                    character.is_whitespace() || character == ':'
                                })
                                .next()
                                .unwrap_or("");
                            if name.is_empty()
                                || !definitions.insert((module.clone(), kind, name.to_owned()))
                            {
                                bail!(
                                    "{}:{start}: duplicate or malformed Lua {kind} {name}",
                                    source.display()
                                );
                            }
                        }
                    }
                    if let Some(symbol) = line.strip_prefix("function ").and_then(|declaration| {
                        declaration.split_once('(').map(|(symbol, _)| symbol)
                    }) {
                        if symbol.is_empty() || !line.ends_with(" end") {
                            bail!(
                                "{}:{start}: malformed Lua callable {symbol}",
                                source.display()
                            );
                        }
                        if !callables.insert((module.clone(), symbol.to_owned())) {
                            bail!(
                                "{}:{start}: duplicate Lua callable {symbol}",
                                source.display()
                            );
                        }
                    }
                }
                let previous = modules.entry(module).or_default().insert(
                    index,
                    Contract {
                        text,
                        source: source.clone(),
                        line: start,
                    },
                );
                if let Some(previous) = previous {
                    bail!(
                        "{}:{start}: contract index {index} already used at {}:{}",
                        source.display(),
                        previous.source.display(),
                        previous.line
                    );
                }
            } else if let Some((_, _, _, payload)) = &mut active {
                let content = line
                    .strip_prefix("//|")
                    .with_context(|| format!("{location}: contract line must start with //|"))?;
                payload.push(content.to_owned());
            }
        }
        if let Some((_, _, line, _)) = active {
            bail!(
                "{}:{line}: unterminated Lua type contract",
                source.display()
            );
        }
    }
    for module in ["mux", "btech"] {
        if !modules.contains_key(module) {
            bail!("no {module} contracts");
        }
    }
    Ok(modules)
}

/// Produce one deterministic LuaLS library file.
fn render(blocks: &BTreeMap<usize, Contract>) -> String {
    let body = blocks
        .values()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "---@meta _\n\n---Generated from Rust LuaLS contracts; run `just update-lua-types`.\n\n{body}\n"
    )
}

/// Compare or atomically replace the production and fixture copies.
fn output(root: &Path, mode: &Mode, module: &str, expected: &str) -> Result<()> {
    for directory in ["game/lua/types", "tests/fixtures/game/lua/types"] {
        let path = root.join(directory).join(format!("{module}.d.lua"));
        match mode {
            Mode::Check => {
                let actual = fs::read_to_string(&path).unwrap_or_default();
                if actual != expected {
                    bail!("{} is stale; run `just update-lua-types`", path.display());
                }
            }
            Mode::Write => {
                if fs::read_to_string(&path).is_ok_and(|actual| actual == expected) {
                    continue;
                }
                fs::create_dir_all(path.parent().context("output has no parent")?)?;
                let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
                use std::io::Write;
                temporary.write_all(expected.as_bytes())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let permissions = fs::metadata(&path)
                        .map(|metadata| metadata.permissions())
                        .unwrap_or_else(|_| fs::Permissions::from_mode(0o644));
                    fs::set_permissions(temporary.path(), permissions)?;
                }
                temporary
                    .persist(&path)
                    .with_context(|| format!("writing {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Validate all contracts before touching any output.
fn main() -> Result<()> {
    let (mode, root) = arguments()?;
    let contracts = contracts(&root)?;
    let rendered = ["mux", "btech"].map(|module| (module, render(&contracts[module])));
    for (module, expected) in rendered {
        output(&root, &mode, module, &expected)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Small fixture checks for parser diagnostics, stable rendering, and stale output.

    use super::*;

    /// Create a minimal source tree with both generated modules represented.
    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("src/lua/packages");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("contracts.rs"),
            "// lua-types-begin mux 00000\n//|function mux.ping() end\n// lua-types-end\n\
             // lua-types-begin btech 00000\n//|function btech.ping() end\n// lua-types-end\n",
        )
        .unwrap();
        directory
    }

    #[test]
    fn contracts_render_in_stable_order_and_check_staleness() {
        let directory = fixture();
        let root = directory.path();
        let all = contracts(root).unwrap();
        let mux = render(&all["mux"]);
        assert!(mux.contains("function mux.ping() end"));
        output(root, &Mode::Write, "mux", &mux).unwrap();
        output(root, &Mode::Check, "mux", &mux).unwrap();
        let path = root.join("game/lua/types/mux.d.lua");
        fs::write(path, "stale").unwrap();
        assert!(output(root, &Mode::Check, "mux", &mux).is_err());
    }

    #[test]
    fn index_gaps_keep_numeric_order() {
        let directory = fixture();
        let source = directory.path().join("src/lua/packages/contracts.rs");
        let original = fs::read_to_string(&source).unwrap();
        fs::write(
            &source,
            format!(
                "// lua-types-begin mux 00009\n//|function mux.last() end\n// lua-types-end\n\
                 {original}\
                 // lua-types-begin mux 00004\n//|function mux.middle() end\n// lua-types-end\n"
            ),
        )
        .unwrap();
        let mux = render(&contracts(directory.path()).unwrap()["mux"]);
        let ping = mux.find("mux.ping").unwrap();
        let middle = mux.find("mux.middle").unwrap();
        let last = mux.find("mux.last").unwrap();
        assert!(ping < middle && middle < last);
    }

    #[test]
    fn malformed_or_duplicate_contracts_fail() {
        let directory = fixture();
        let source = directory.path().join("src/lua/packages/contracts.rs");
        let original = fs::read_to_string(&source).unwrap();
        fs::write(&source, format!("{original}// lua-types-begin mux 00000\n//|function mux.ping() end\n// lua-types-end\n")).unwrap();
        assert!(contracts(directory.path()).is_err());
        fs::write(
            &source,
            original.replace("//|function mux.ping() end", "bad payload line"),
        )
        .unwrap();
        assert!(contracts(directory.path()).is_err());
    }
}
