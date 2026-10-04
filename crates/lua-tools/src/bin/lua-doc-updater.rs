//! Generate and check the Hugo Lua API reference from Rust-owned contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use regex::{Captures, Regex};

/// One callable declared by the generated LuaLS libraries.
struct ApiFunction {
    name: String,
    path: String,
    parameters: String,
    comments: Vec<String>,
}

/// A documented typed constant and its source value or type.
#[derive(Clone)]
struct Constant {
    name: String,
    value: String,
    detail: String,
}

/// Select generation or verification, like `lua-type-updater`.
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

/// Parse the small CLI shared in style with the Lua definition updater.
fn arguments() -> Result<(Mode, PathBuf)> {
    let mut mode = None;
    let mut root = repository_root();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" if mode.is_none() => mode = Some(Mode::Check),
            "--write" if mode.is_none() => mode = Some(Mode::Write),
            "--repo-root" => root = PathBuf::from(args.next().context("--repo-root needs a path")?),
            _ => bail!("unknown or repeated option: {arg}"),
        }
    }
    Ok((
        mode.context("specify exactly one of --check or --write")?,
        root,
    ))
}

/// Compile a static pattern with an actionable error if its syntax is changed.
fn pattern(expression: &str) -> Regex {
    Regex::new(expression).expect("valid Lua reference pattern")
}

/// Replace LuaLS links and escape table-column separators.
fn clean(value: &str) -> String {
    let links = pattern(r"\[([^]]+)\]\(lua://[^)]+\)");
    links
        .replace_all(value, |captures: &Captures<'_>| {
            format!("`{}`", captures[1].trim_matches('`'))
        })
        .replace('|', "\\|")
}

/// Split a LuaLS type from prose after nested generic and record brackets.
fn split_type(value: &str) -> (&str, &str) {
    let mut depth: i32 = 0;
    for (index, character) in value.char_indices() {
        match character {
            '<' | '{' | '[' | '(' => depth += 1,
            '>' | '}' | ']' | ')' => depth -= 1,
            ' ' if depth == 0 => return (&value[..index], &value[index + 1..]),
            _ => {}
        }
    }
    (value, "")
}

/// Change Lua snake-case names to Hugo page slugs.
fn slug(name: &str) -> String {
    name.replace('_', "-")
}

/// Extract the callable name without its package or receiver.
fn function_name(function: &ApiFunction) -> &str {
    function
        .name
        .rsplit([':', '.'])
        .next()
        .expect("nonempty callable name")
}

/// Locate the output path of a callable's page.
fn function_page(function: &ApiFunction) -> String {
    format!("{}/{}.md", function.path, slug(function_name(function)))
}

/// Wrap generated Markdown in the site's page front matter.
fn page(title: &str, link_title: Option<&str>, body: &str, index: bool) -> String {
    let mut output = format!("---\ntitle: \"{title}\"\ntype: docs\n");
    if let Some(link_title) = link_title {
        output.push_str(&format!("linkTitle: \"{link_title}\"\n"));
        output.push_str(&format!("manualLinkTitle: \"{link_title}\"\n"));
    }
    if index {
        output.push_str("no_list: true\n");
    }
    output.push_str("---\n\n");
    output.push_str(body.trim_end());
    output.push('\n');
    output
}

/// Keep section navigation local to the package, following the source site's layout.
fn index_page(path: &str, body: &str) -> String {
    let (title, link_title, weight) = match path {
        "" => ("Package Reference".to_owned(), None, Some(1000)),
        "mux" => ("mux package".to_owned(), Some("mux".to_owned()), Some(10)),
        "btech" => (
            "btech package".to_owned(),
            Some("btech".to_owned()),
            Some(20),
        ),
        _ => {
            let title = path.replace('/', ".");
            let link_title = match path {
                "mux/comsys/type-channel" => "Channel".to_owned(),
                "mux/comsys/type-channel-flags" => "ChannelFlags".to_owned(),
                "mux/world/type-flags" => "Flags".to_owned(),
                "mux/world/type-object" => "Object".to_owned(),
                "mux/world/type-powers" => "Powers".to_owned(),
                "mux/world/type-state" => "State".to_owned(),
                _ => title.clone(),
            };
            let title = if path.contains("/type-") {
                link_title.clone()
            } else {
                title
            };
            (title, Some(link_title), index_weight(path))
        }
    };
    let mut output = format!("---\ntitle: \"{title}\"\n");
    if let Some(link_title) = link_title {
        output.push_str(&format!("linkTitle: \"{link_title}\"\n"));
    }
    if path.is_empty() {
        output.push_str("description: A reference for the Lua scripting APIs\n");
    }
    output.push_str("type: docs\n");
    if let Some(weight) = weight {
        output.push_str(&format!("weight: {weight}\n"));
    }
    if !path.is_empty() {
        output.push_str("sidebar_root_for: self\n");
    }
    output.push_str("no_list: true\n---\n\n");
    output.push_str(body.trim_end());
    output.push('\n');
    output
}

/// Preserve the familiar ordering of sections that existed in the source site.
fn index_weight(path: &str) -> Option<i32> {
    match path {
        "btech/autopilot" => Some(5),
        "btech/character" => Some(10),
        "btech/error" | "mux/error" => Some(-50),
        "btech/map" => Some(-10),
        "btech/parts" => Some(0),
        "btech/player" => Some(-5),
        "btech/repair" | "mux/comsys/type-channel" | "mux/world/type-flags" => Some(20),
        "btech/system" | "mux/comsys/type-channel-flags" | "mux/world/type-object" => Some(30),
        "btech/template" | "mux/config" => Some(15),
        "btech/unit" | "mux/world" => Some(-20),
        "mux/comsys" => Some(14),
        "mux/session" => Some(-35),
        "mux/telnet" => Some(-40),
        "mux/text" => Some(-30),
        "mux/world/type-powers" => Some(40),
        "mux/world/type-state" => Some(50),
        _ => None,
    }
}

/// One-line descriptions for the package tables and subpackage introductions.
fn subpackage_description(path: &str) -> Option<&'static str> {
    match path {
        "mux/macro" => Some("Trusted macro-set management, permissions, and player attachments."),
        "mux/comsys" => Some("Trusted communication-channel management."),
        "mux/config" => Some("Read-only access to scalar server configuration."),
        "mux/error" => Some("Structured errors, checked error codes, and error-handling helpers."),
        "mux/session" => Some("Interactive flows and active player-session information."),
        "mux/telnet" => Some("Telnet protocol state and capabilities."),
        "mux/text" => Some("Styled-text validation, formatting, and measurement helpers."),
        "mux/world" => Some("Database objects and their persistent state."),
        "btech/autopilot" => Some("Lua control of unit-attached ground autopilots."),
        "btech/tactical" => Some("Filtered friendly-force snapshots and atomic unit intentions."),
        "btech/cargo" => Some("Cockpit stock reports and cargo transfers."),
        "btech/character" => Some("Character values, skills, and experience."),
        "btech/database" => Some("Explicit BattleTech world checkpoints."),
        "btech/error" => Some("Checked BattleTech error-code symbols."),
        "btech/inventory" => Some("Loose-parts inventory and stock changes."),
        "btech/map" => Some("Maps, geometry, line of sight, placement, and messaging."),
        "btech/parts" => Some("Part catalogue and stores."),
        "btech/player" => Some("Saved player configuration and preferences."),
        "btech/repair" => Some("Immediate repairs and technician scheduling."),
        "btech/runtime" => Some("Wizard runtime diagnostics."),
        "btech/system" => Some("Server-wide BattleTech queries."),
        "btech/template" => Some("Unit-template inspection and displays."),
        "btech/unit" => Some("Live-unit state, combat queries, and mutations."),
        "btech/weapon" => Some("Runtime weapon settings."),
        _ => None,
    }
}

/// Map a LuaLS method receiver to its existing reference section.
fn class_path(owner: &str) -> Option<&'static str> {
    match owner {
        "MacroSet" => Some("mux/macro/type-set"),
        "MacroFlags" => Some("mux/macro/type-flags"),
        "Channel" => Some("mux/comsys/type-channel"),
        "ChannelFlags" => Some("mux/comsys/type-channel-flags"),
        "Error" => Some("mux/error/type-error"),
        "Flags" => Some("mux/world/type-flags"),
        "Object" => Some("mux/world/type-object"),
        "Powers" => Some("mux/world/type-powers"),
        "State" => Some("mux/world/type-state"),
        _ => None,
    }
}

/// Map LuaLS constant classes to their public namespace.
fn mux_namespace(class: &str) -> Option<&'static str> {
    match class {
        "AccessNamespace" => Some("mux/world/access"),
        "FlagNamespace" => Some("mux/world/flags"),
        "LockNamespace" => Some("mux/world/locks"),
        "PowerNamespace" => Some("mux/world/powers"),
        "ObjectTypeNamespace" => Some("mux/world/types"),
        "MacroFlagConstants" => Some("mux/macro/flags"),
        "ChannelFlagNamespace" => Some("mux/comsys/flags"),
        _ => None,
    }
}

/// Read callable declarations and their adjacent LuaLS annotations.
fn parse_functions(source: &Path) -> Result<Vec<ApiFunction>> {
    let function = pattern(r"^function ([A-Za-z_][\w]*)([.:])([\w]+)\((.*)\) end$");
    let mut result = Vec::new();
    let mut comments = Vec::new();
    for line in fs::read_to_string(source)?.lines() {
        if line.starts_with("---") {
            comments.push(line.to_owned());
            continue;
        }
        if let Some(found) = function.captures(line) {
            let owner = &found[1];
            let name = &found[3];
            let (path, public_name) = if let Some(path) = class_path(owner) {
                (path.to_owned(), format!("{owner}:{name}"))
            } else {
                let (root, package) = owner.split_once('_').unwrap_or((owner, ""));
                if root != "mux" && root != "btech" {
                    bail!("{}: unknown Lua owner {owner}", source.display());
                }
                let path = if package.is_empty() {
                    root.to_owned()
                } else {
                    format!("{root}/{package}")
                };
                let public_name = if package.is_empty() {
                    format!("{root}.{name}")
                } else {
                    format!("{root}.{package}.{name}")
                };
                (path, public_name)
            };
            result.push(ApiFunction {
                name: public_name,
                path,
                parameters: found[4].to_owned(),
                comments: std::mem::take(&mut comments),
            });
        }
        comments.clear();
    }
    Ok(result)
}

/// Read callable fields from the typed autopilot facade.
///
/// Most Lua APIs are declared as `function` statements in the LuaLS library.
/// The autopilot facade is represented as a typed API record instead, because
/// its functions are installed on a table at runtime. Keep the generated
/// reference complete without requiring a second, hand-maintained signature
/// inventory.
fn parse_autopilot_functions(source: &Path) -> Result<Vec<ApiFunction>> {
    let mut in_api = false;
    let mut result = Vec::new();
    for line in fs::read_to_string(source)?.lines() {
        if let Some(class) = class_name(line) {
            in_api = class == "BtechAutopilotAPI";
            continue;
        }
        if !in_api {
            continue;
        }
        let Some(rest) = line.strip_prefix("---@field ") else {
            if !line.starts_with("---") {
                in_api = false;
            }
            continue;
        };
        let Some((name, declaration)) = rest.split_once(' ') else {
            continue;
        };
        let Some(signature) = declaration.strip_prefix("fun(") else {
            continue;
        };
        let Some((parameters, return_type)) = signature.split_once(')') else {
            bail!("{}: malformed autopilot callable {name}", source.display());
        };
        let parameter_types = parameters
            .split(',')
            .filter_map(|parameter| {
                parameter.trim().split_once(':').map(|(name, kind)| {
                    (
                        name.trim().trim_end_matches('?').to_owned(),
                        kind.trim().to_owned(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let parameters = parameter_types
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let return_type = return_type
            .strip_prefix(": ")
            .or_else(|| return_type.strip_prefix(":"));
        let mut comments = Vec::new();
        let (summary, params, returns) = match name {
            "attach" => (
                "Attach a paused ground autopilot controller to a supported Mech or ground vehicle.",
                vec![
                    ("unit", "The unit to control."),
                    ("options", "Optional initial configuration."),
                ],
                None,
            ),
            "detach" => (
                "Detach the unit's controller, stop autonomous movement, and clear its durable state.",
                vec![("unit", "The controlled unit.")],
                None,
            ),
            "configure" => (
                "Atomically update controller settings and return the new management revision.",
                vec![
                    ("unit", "The controlled unit."),
                    ("patch", "Settings to change."),
                    ("expected_revision", "Optional revision guard."),
                ],
                Some("integer New management revision."),
            ),
            "submit" => (
                "Validate and queue typed movement or combat orders.",
                vec![
                    ("unit", "The controlled unit."),
                    ("orders", "Order specification tables."),
                    ("mode", "Append or replace the existing queue."),
                    ("expected_revision", "Optional revision guard."),
                ],
                Some("AutopilotSubmitResult Order IDs and the new management revision."),
            ),
            "cancel" => (
                "Cancel an active or queued order.",
                vec![
                    ("unit", "The controlled unit."),
                    ("order_id", "The stable order ID."),
                    ("expected_revision", "Optional revision guard."),
                ],
                Some("boolean True when an order was canceled."),
            ),
            "pause" => (
                "Pause the controller and request a normal movement stop.",
                vec![("unit", "The controlled unit.")],
                None,
            ),
            "resume" => (
                "Resume a paused controller after rechecking its unit and order prerequisites.",
                vec![("unit", "The controlled unit.")],
                None,
            ),
            "status" => (
                "Read detached controller settings, state, orders, progress, and blocking information.",
                vec![("unit", "The controlled unit.")],
                Some("AutopilotStatus Controller status."),
            ),
            "observe" => (
                "Read the controller's filtered own-unit, visible-contact, and remembered-sighting observations.",
                vec![("unit", "The controlled unit.")],
                Some("AutopilotObservation Filtered observation snapshot."),
            ),
            "feedback" => (
                "Read bounded order transitions and outcome records.",
                vec![
                    ("unit", "The controlled unit."),
                    ("after_sequence", "Optional sequence cursor."),
                ],
                Some("AutopilotFeedbackPage Feedback records and a history-gap indicator."),
            ),
            _ => continue,
        };
        comments.push(format!("---{summary}"));
        for (parameter, detail) in params {
            let kind = parameter_types
                .iter()
                .find(|(name, _)| name == parameter)
                .map_or("any", |(_, kind)| kind.as_str());
            comments.push(format!("---@param {parameter} {kind} {detail}"));
        }
        let return_value = returns.or_else(|| {
            let value = return_type?.trim();
            (!value.is_empty()).then_some(value)
        });
        if let Some(return_value) = return_value {
            comments.push(format!("---@return {return_value}"));
        }
        result.push(ApiFunction {
            name: format!("btech.autopilot.{name}"),
            path: "btech/autopilot".to_owned(),
            parameters,
            comments,
        });
    }
    Ok(result)
}

/// Turn a callable's annotations and optional example into its reference page body.
fn function_body(function: &ApiFunction, example: Option<&str>) -> String {
    let annotation = pattern(r"^---@(param|return|see)\s+(.*)$");
    let mut prose = Vec::new();
    let mut parameters = Vec::new();
    let mut returns = Vec::new();
    let mut errors = Vec::new();
    for line in &function.comments {
        if let Some(found) = annotation.captures(line) {
            let value = &found[2];
            match &found[1] {
                "param" => {
                    let (name, rest) = value.split_once(' ').unwrap_or((value, ""));
                    let (kind, detail) = split_type(rest);
                    parameters.push((
                        name.to_owned(),
                        if kind.is_empty() { "any" } else { kind }.to_owned(),
                        detail.to_owned(),
                    ));
                }
                "return" => returns.push(value.to_owned()),
                _ => errors.push(value.to_owned()),
            }
        } else if let Some(comment) = line.strip_prefix("---")
            && !line.starts_with("---@")
        {
            prose.push(clean(comment.trim_start()));
        }
    }
    let joined = prose.join("\n");
    let normalized = pattern(r"\n{3,}").replace_all(joined.trim(), "\n\n");
    let summary = if normalized.is_empty() {
        "Callable provided by the Stompymux-rs Lua runtime."
    } else {
        normalized.as_ref()
    };
    let mut body = format!(
        "{summary}\n\n## Signature\n\n```lua\n{}({})\n```\n\n## Parameters\n\n",
        function.name, function.parameters
    );
    if parameters.is_empty() {
        body.push_str("None.\n");
    } else {
        body.push_str("| Name | Type | Description |\n| --- | --- | --- |\n");
        for (name, kind, detail) in parameters {
            body.push_str(&format!(
                "| `{name}` | `{}` | {} |\n",
                clean(&kind),
                clean(&detail)
            ));
        }
    }
    body.push_str("\n## Returns\n\n");
    if returns.is_empty() {
        body.push_str("No values.\n");
    } else {
        for value in returns {
            body.push_str(&format!("- `{}`\n", clean(&value).replace("\\|", "|")));
        }
    }
    if !errors.is_empty() {
        body.push_str("\n## Related errors\n\n");
        let mut seen = BTreeSet::new();
        for error in errors {
            if seen.insert(error.clone()) {
                body.push_str(&format!("- `{error}`\n"));
            }
        }
    }
    if let Some(example) = example {
        body.push_str(&format!(
            "\n## Example\n\n```lua\n{}\n```\n",
            example.trim_end()
        ));
    }
    body
}

/// Extract class names from LuaLS annotations.
fn class_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("---@class ")?;
    let rest = rest.strip_prefix("(exact) ").unwrap_or(rest);
    Some(rest.split([' ', ':']).next().unwrap_or(""))
}

/// Extract a LuaLS field and its type and description.
fn field(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix("---@field ")?;
    let (name, description) = rest.split_once(' ')?;
    let (kind, detail) = split_type(description);
    Some((name.trim_end_matches('?'), kind, detail))
}

/// Extract MUX typed constant names and descriptions from LuaLS classes.
fn parse_mux_constants(source: &Path) -> Result<BTreeMap<String, Vec<Constant>>> {
    let mut result = BTreeMap::new();
    let mut current: Option<&str> = None;
    for line in fs::read_to_string(source)?.lines() {
        if let Some(class) = class_name(line) {
            current = mux_namespace(class);
            if let Some(path) = current {
                result.insert(path.to_owned(), Vec::new());
            }
            continue;
        }
        if let (Some(path), Some((name, value, detail))) = (current, field(line)) {
            let entries: &mut Vec<Constant> = result.get_mut(path).expect("namespace initialized");
            if !entries.iter().any(|entry| entry.name == name) {
                entries.push(Constant {
                    name: name.to_owned(),
                    value: value.to_owned(),
                    detail: clean(detail),
                });
            }
        } else if !line.starts_with("---") {
            current = None;
        }
    }
    Ok(result)
}

/// Extract BTech typed constant catalogs from the Rust registry.
fn parse_btech_constants(source: &Path) -> Result<BTreeMap<String, Vec<Constant>>> {
    let input = fs::read_to_string(source)?;
    let catalog = pattern(r#"(?s)qualified_name: "(btech\.[\w.]+)",\s*entries: &\[(.*?)\],"#);
    let entry = pattern(
        r#"(?s)(?:String)?Entry\s*\{\s*name: "([\w]+)",\s*value: (?:"([\w]+)"|(\d+)),\s*\}"#,
    );
    let mut result = BTreeMap::new();
    for found in catalog.captures_iter(&input) {
        let path = found[1].replace('.', "/");
        let fields = entry
            .captures_iter(&found[2])
            .map(|item| Constant {
                name: item[1].to_owned(),
                value: item
                    .get(2)
                    .map(|value| format!("\"{}\"", value.as_str()))
                    .unwrap_or_else(|| item[3].to_owned()),
                detail: String::new(),
            })
            .collect();
        result.insert(path, fields);
    }
    Ok(result)
}

/// Extract checked error-code trees from their native catalog.
fn parse_error_codes(source: &Path) -> Result<BTreeMap<String, Vec<Constant>>> {
    let input = fs::read_to_string(source)?;
    let code = pattern(r#""((?:mux|btech)\.[\w.]+)""#);
    let mut result = BTreeMap::new();
    for root in ["mux", "btech"] {
        let fields: Vec<_> = code
            .captures_iter(&input)
            .filter_map(|found| {
                let value = found[1].to_owned();
                let name = value.strip_prefix(&format!("{root}."))?.to_owned();
                Some(Constant {
                    name,
                    value,
                    detail: String::new(),
                })
            })
            .collect();
        result.insert(format!("{root}/error/codes"), fields.clone());
        if root == "btech" {
            result.insert("btech/errors".to_owned(), fields);
        }
    }
    Ok(result)
}

/// Render the named record and alias shapes used by callable signatures.
fn type_body(source: &Path) -> Result<String> {
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut active: Option<usize> = None;
    for line in fs::read_to_string(source)?.lines() {
        if let Some(name) = class_name(line) {
            active = Some(blocks.len());
            blocks.push((name.to_owned(), Vec::new()));
        } else if let Some(alias) = line.strip_prefix("---@alias ") {
            active = None;
            let (name, value) = alias.split_once(' ').unwrap_or((alias, ""));
            blocks.push((
                name.to_owned(),
                vec![format!("Alias: `{}`", clean(value).replace("\\|", "|"))],
            ));
        } else if let (Some(index), Some((name, kind, detail))) = (active, field(line)) {
            let mut rendered = format!("- `{name}`: `{}`", clean(kind).replace("\\|", "|"));
            if !detail.is_empty() {
                rendered.push_str(&format!(" — {}", clean(detail)));
            }
            blocks[index].1.push(rendered);
        } else if !line.starts_with("---") {
            active = None;
        }
    }
    let mut body =
        "Record shapes and aliases used by the callable signatures in this reference.\n\n"
            .to_owned();
    for (name, fields) in blocks {
        if fields.is_empty() {
            continue;
        }
        body.push_str(&format!("## {name}\n\n{}\n\n", fields.join("\n")));
    }
    Ok(body)
}

/// Load hand-authored Lua examples keyed by their generated Markdown page.
fn examples(directory: &Path) -> Result<BTreeMap<String, String>> {
    fn visit(directory: &Path, root: &Path, result: &mut BTreeMap<String, String>) -> Result<()> {
        if !directory.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                visit(&path, root, result)?;
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "lua") {
                bail!("Lua example must have a .lua extension: {}", path.display());
            }
            let relative = path.strip_prefix(root)?.with_extension("md");
            let name = relative.to_string_lossy().replace('\\', "/");
            let source = fs::read_to_string(&path)?;
            if source.trim().is_empty() {
                bail!("Lua example is empty: {}", path.display());
            }
            result.insert(name, source);
        }
        Ok(())
    }

    let mut result = BTreeMap::new();
    visit(directory, directory, &mut result)?;
    Ok(result)
}

/// Render a complete set of generated Hugo files in memory.
fn render(root: &Path) -> Result<BTreeMap<String, String>> {
    let types = root.join("game/lua/types");
    let mut functions = parse_functions(&types.join("mux.d.lua"))?;
    functions.extend(parse_functions(&types.join("btech.d.lua"))?);
    functions.extend(parse_autopilot_functions(&types.join("btech.d.lua"))?);
    let mut constants = parse_mux_constants(&types.join("mux.d.lua"))?;
    constants.extend(parse_btech_constants(
        &root.join("src/lua/packages/btech/constants.rs"),
    )?);
    constants.extend(parse_error_codes(
        &root.join("src/lua/packages/error/catalog.rs"),
    )?);
    let examples = examples(&root.join("docs/lua-examples"))?;
    let callable_pages: BTreeSet<_> = functions.iter().map(function_page).collect();
    for path in examples.keys() {
        if !callable_pages.contains(path) {
            bail!("Lua example has no matching callable page: {path}");
        }
    }

    let mut pages = BTreeMap::new();
    let mut groups: BTreeMap<String, Vec<&ApiFunction>> = BTreeMap::new();
    for function in &functions {
        groups
            .entry(function.path.clone())
            .or_default()
            .push(function);
        let path = function_page(function);
        if pages
            .insert(
                path.clone(),
                page(
                    &function.name,
                    Some(function_name(function)),
                    &function_body(function, examples.get(&path).map(String::as_str)),
                    false,
                ),
            )
            .is_some()
        {
            bail!("duplicate reference page: {path}");
        }
    }

    for (path, fields) in &constants {
        let string_catalog = path.starts_with("btech/autopilot/")
            && fields.iter().all(|field| field.value.starts_with('"'));
        let mut body = if string_catalog {
            "Immutable string constants available in the Stompymux-rs Lua runtime.\n\n"
        } else {
            "Immutable typed constants available in the Stompymux-rs Lua runtime.\n\n"
        }
        .to_owned();
        if path.starts_with("btech/") && !path.ends_with("codes") && !path.ends_with("errors") {
            if string_catalog {
                body.push_str(
                    "The values below are serialized strings returned by this namespace.\n\n",
                );
            } else {
                body.push_str("The numbers below are native identifiers; pass the typed constants to Lua APIs.\n\n");
            }
        }
        body.push_str("| Constant | Type or native code | Description |\n| --- | --- | --- |\n");
        for field in fields {
            body.push_str(&format!(
                "| `{}.{}` | `{}` | {} |\n",
                path.replace('/', "."),
                field.name,
                field.value,
                field.detail
            ));
        }
        pages.insert(
            format!("{path}.md"),
            page(&path.replace('/', "."), None, &body, false),
        );
    }

    for package in ["mux", "btech"] {
        pages.insert(
            format!("{package}/types.md"),
            page(
                &format!("{package} value types"),
                None,
                &type_body(&types.join(format!("{package}.d.lua")))?,
                false,
            ),
        );
    }

    let mut paths: BTreeSet<String> = ["mux", "btech"].into_iter().map(str::to_owned).collect();
    paths.extend(groups.keys().cloned());
    for path in constants.keys() {
        paths.insert(
            path.rsplit_once('/')
                .map_or(path.as_str(), |(parent, _)| parent)
                .to_owned(),
        );
    }
    let descendants: Vec<_> = paths.iter().cloned().collect();
    for path in descendants {
        let parts: Vec<_> = path.split('/').collect();
        for index in 1..parts.len() {
            paths.insert(parts[..index].join("/"));
        }
    }
    for path in &paths {
        let title = path.replace('/', ".");
        let mut body = match path.as_str() {
            "mux" => "`mux` is the built-in server API available to every Lua module. It is supplied by the game server rather than loaded with `require`.\n\n".to_owned(),
            "btech" => "`require(\"btech\")` returns the native, typed BattleTech API. Gameplay calls are unavailable during `@lua/check`.\n\n".to_owned(),
            _ => match subpackage_description(path) {
                Some(description) => format!("`{title}`: {description}\n\n"),
                None => format!("`{title}` is part of the Lua API.\n\n"),
            },
        };
        let children: Vec<_> = paths
            .iter()
            .filter(|child| direct_child(path, child))
            .collect();
        if !children.is_empty() {
            if path == "mux" || path == "btech" {
                body.push_str("## Subpackages\n\n| Package | Description |\n| --- | --- |\n");
                for child in children {
                    let description = subpackage_description(child)
                        .with_context(|| format!("missing subpackage description for {child}"))?;
                    let short = child.rsplit('/').next().expect("child segment");
                    body.push_str(&format!(
                        "| [`{}`]({short}/) | {description} |\n",
                        child.replace('/', ".")
                    ));
                }
            } else {
                body.push_str("## Namespaces and types\n\n");
                for child in children {
                    let short = child.rsplit('/').next().expect("child segment");
                    body.push_str(&format!("- [`{}`]({short}/)\n", child.replace('/', ".")));
                }
            }
            body.push('\n');
        }
        if let Some(symbols) = groups.get(path) {
            body.push_str("## Functions\n\n");
            let mut symbols = symbols.clone();
            symbols.sort_by(|a, b| a.name.cmp(&b.name));
            for function in symbols {
                let short = function_name(function);
                body.push_str(&format!("- [`{short}`]({}/)\n", slug(short)));
            }
        }
        let constant_children: Vec<_> = constants
            .keys()
            .filter(|child| direct_child(path, child))
            .collect();
        if !constant_children.is_empty() {
            body.push_str("\n## Constants\n\n");
            for child in constant_children {
                let short = child.rsplit('/').next().expect("constant segment");
                body.push_str(&format!("- [`{}`]({short}/)\n", child.replace('/', ".")));
            }
        }
        if path == "btech/autopilot" {
            body.push_str("\nAttach a controller to a ground unit, submit typed orders, and inspect its observations and feedback. Controllers resume persisted work after restart.\n");
        }
        if path == "btech" {
            body.push_str("\nFunctions require a live game callback unless their page states otherwise. `DbRef` is an integer object reference; `Object` is a live world handle.\n");
        }
        if path == "mux" || path == "btech" {
            body.push_str(
                "\nSee the [value types](types/) used in signatures and returned records.\n",
            );
        }
        pages.insert(format!("{path}/_index.md"), index_page(path, &body));
    }

    let root = "| Package | Description |\n| --- | --- |\n\
| [`mux`](mux/) | World objects, communication, text, sessions, and errors. |\n\
| [`btech`](btech/) | BattleTech maps, units, equipment, and operations. |\n";
    pages.insert("_index.md".to_owned(), index_page("", root));
    println!(
        "Rendered {} function pages and {} constant pages",
        functions.len(),
        constants.len()
    );
    Ok(pages)
}

/// Test whether `child` is exactly one section beneath `parent`.
fn direct_child(parent: &str, child: &str) -> bool {
    child
        .strip_prefix(parent)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|rest| !rest.contains('/'))
}

/// Recursively collect generated Markdown paths for verification.
fn actual_pages(directory: &Path, root: &Path, files: &mut BTreeSet<String>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            actual_pages(&path, root, files)?;
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.insert(
                path.strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

/// Compare or replace the generated reference in its dedicated directory.
fn update(root: &Path, mode: Mode, pages: &BTreeMap<String, String>) -> Result<()> {
    let destination = root.join("docs/content/docs/scripting/packages");
    match mode {
        Mode::Check => {
            let mut actual = BTreeSet::new();
            actual_pages(&destination, &destination, &mut actual)?;
            let expected: BTreeSet<_> = pages.keys().cloned().collect();
            if actual != expected {
                let missing: Vec<_> = expected.difference(&actual).collect();
                let extra: Vec<_> = actual.difference(&expected).collect();
                bail!("Lua reference page set is stale; missing: {missing:?}; extra: {extra:?}");
            }
            for (path, contents) in pages {
                if fs::read_to_string(destination.join(path))? != *contents {
                    bail!("Lua reference page is stale: {path}");
                }
            }
            println!("Lua reference is current ({} pages)", pages.len());
        }
        Mode::Write => {
            if destination.exists() {
                fs::remove_dir_all(&destination)
                    .with_context(|| format!("removing {}", destination.display()))?;
            }
            for (path, contents) in pages {
                let output = destination.join(path);
                fs::create_dir_all(output.parent().expect("page parent"))?;
                fs::write(&output, contents)
                    .with_context(|| format!("writing {}", output.display()))?;
            }
            println!("Wrote {} Lua reference pages", pages.len());
        }
    }
    Ok(())
}

/// Render and update the checked-in Lua reference.
fn main() -> Result<()> {
    let (mode, root) = arguments()?;
    let pages = render(&root)?;
    update(&root, mode, &pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_nested_luals_types_without_losing_descriptions() {
        assert_eq!(
            split_type("table<integer, CriticalDefinition> Zero-based slots."),
            ("table<integer, CriticalDefinition>", "Zero-based slots.")
        );
        assert_eq!(split_type("DbRef|Object"), ("DbRef|Object", ""));
    }

    #[test]
    fn checks_exact_section_children() {
        assert!(direct_child("btech/unit", "btech/unit/sections"));
        assert!(!direct_child("btech/unit", "btech/unit/sections/left"));
        assert!(!direct_child("btech/unit", "btech/unitish/sections"));
    }

    #[test]
    fn check_detects_stale_generated_content() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let pages = BTreeMap::from([("mux/_index.md".to_owned(), "current\n".to_owned())]);
        update(directory.path(), Mode::Write, &pages)?;
        update(directory.path(), Mode::Check, &pages)?;
        let page = directory
            .path()
            .join("docs/content/docs/scripting/packages/mux/_index.md");
        fs::write(page, "stale\n")?;
        assert!(update(directory.path(), Mode::Check, &pages).is_err());
        Ok(())
    }

    #[test]
    fn example_is_rendered_after_the_contract() {
        let function = ApiFunction {
            name: "Object:name".to_owned(),
            path: "mux/world/type-object".to_owned(),
            parameters: String::new(),
            comments: vec!["---Returns the object's name.".to_owned()],
        };
        let body = function_body(&function, Some("local name = object:name()\n"));
        assert!(body.contains("## Example\n\n```lua\nlocal name = object:name()\n```"));
        assert!(body.find("## Returns").unwrap() < body.find("## Example").unwrap());
    }

    #[test]
    fn callable_pages_use_the_short_name_in_navigation() {
        let function = ApiFunction {
            name: "btech.unit.set_max_speed".to_owned(),
            path: "btech/unit".to_owned(),
            parameters: "unit, speed".to_owned(),
            comments: Vec::new(),
        };
        let rendered = page(
            &function.name,
            Some(function_name(&function)),
            &function_body(&function, None),
            false,
        );
        assert!(rendered.starts_with(
            "---\ntitle: \"btech.unit.set_max_speed\"\ntype: docs\nlinkTitle: \"set_max_speed\"\nmanualLinkTitle: \"set_max_speed\"\n"
        ));
    }

    #[test]
    fn section_lists_use_short_callable_names() -> Result<()> {
        let pages = render(&repository_root())?;
        let unit_index = &pages["btech/unit/_index.md"];
        assert!(unit_index.contains("- [`armor`](armor/)"));
        assert!(!unit_index.contains("- [`btech.unit.armor`](armor/)"));
        assert!(unit_index.contains("sidebar_root_for: self\nno_list: true"));
        assert!(unit_index.contains("weight: -20"));
        let mux = &pages["mux/_index.md"];
        assert!(
            mux.contains(
                "| [`mux.world`](world/) | Database objects and their persistent state. |"
            )
        );
        assert!(mux.contains("weight: 10\nsidebar_root_for: self\nno_list: true"));
        let btech = &pages["btech/_index.md"];
        assert!(btech.contains(
            "| [`btech.autopilot`](autopilot/) | Lua control of unit-attached ground autopilots. |"
        ));
        assert!(
            btech.contains(
                "| [`btech.cargo`](cargo/) | Cockpit stock reports and cargo transfers. |"
            )
        );
        let package_root = &pages["_index.md"];
        assert!(package_root.contains("weight: 1000\nno_list: true"));
        Ok(())
    }

    #[test]
    fn autopilot_typed_fields_get_reference_pages() -> Result<()> {
        let pages = render(&repository_root())?;
        let index = &pages["btech/autopilot/_index.md"];
        assert!(index.contains("- [`attach`](attach/)"));
        assert!(index.contains("- [`feedback`](feedback/)"));
        let attach = &pages["btech/autopilot/attach.md"];
        assert!(attach.contains("btech.autopilot.attach(unit, options)"));
        assert!(attach.contains("supported Mech or ground vehicle"));
        assert!(pages["btech/autopilot/states.md"].contains("`\"paused\"`"));
        assert!(pages["btech/autopilot/order_states.md"].contains("`\"running\"`"));
        assert!(pages["btech/autopilot/reasons.md"].contains("`\"contact_lost\"`"));
        Ok(())
    }
}
