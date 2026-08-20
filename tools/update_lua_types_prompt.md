Review the native Lua packages and update their checked-in LuaLS definition
files.

Scope and authority:

- Discover the public `mux` and `btech` API from the registration code under
  `src/mux/lua/packages/`.
- Treat the Doxygen comments immediately above bound C functions and their
  actual implementations as authoritative. Runtime behavior wins if comments
  or scripting documentation disagree.
- Follow every `BTECH_LUA_ENTRIES` handler into `src/btech/scripting/`.
- Cross-check stable errors in `src/mux/lua/lua_error_codes.h` and
  `src/mux/lua/lua_error.c`, including errors raised by shared helpers.
- Use `docs/content/docs/scripting/packages/` to clarify public terminology,
  descriptions, examples, overloads, and legacy behavior that is ambiguous in
  C. Do not edit those documents.

Update only these artifacts:

- `game/lua/types/mux.d.lua`
- `game/lua/types/btech.d.lua`

Both files must be valid LuaLS definition files beginning with `---@meta _`.
Follow it with a comment saying the file is maintained by
`just update-lua-types` and should be refreshed from the native bindings and
their Doxygen comments rather than edited alone.
Describe every public symbol with a useful prose comment and detailed LuaCATS
annotations. Include package globals, classes, fields, aliases, parameters,
returns, overloads, table shapes, literal unions, operators, and optional
values where supported by the implementation. Model `Error`, checked error
code nodes, `mux.error.codes`, and `btech.error.codes`. Because LuaCATS has no
`@throws`, give affected functions a `Raises` paragraph with Markdown symbol
links and `@see` annotations for the corresponding checked code nodes.
Adhere to the canonical LuaCATS annotation syntax documented at
https://luals.github.io/wiki/annotations/; do not invent annotation modifiers,
scopes, or type syntax that the reference does not define.

Preserve unrelated working-tree changes. Do not edit C, documentation,
configuration, tests, or any file outside `game/lua/types/`. Make reasonable
best-effort types where a legacy handler is ambiguous. Run StyLua on the two
definition files when finished. Then run the Lua Language Server diagnosis
validator over `game/lua/types` with warnings enabled (equivalent to
`lua-language-server --check game/lua/types --checklevel=Warning`) and inspect
its report; correct every syntax, annotation, duplicate-definition, and type
diagnostic caused by the generated stubs. If `lua-language-server` is not on
`PATH`, locate the executable bundled with the installed VS Code Lua extension
(for example under `~/.vscode/extensions/sumneko.lua-*/server/bin/`). Do not
consider the update complete if the validator is unavailable or reports any
diagnostic. Also run `luac -p` on both files.
Finally, review the resulting diff for accidental API omissions or unrelated
changes.
