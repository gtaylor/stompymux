# Global Lua logic

Put globally matched command and scheduled-logic modules here. Files are
discovered recursively and run in lexical relative-path order. A command
handler returning `true` stops global dispatch; `false` or `nil` allows the
next handler to try.

Use domain-oriented paths such as `player/help.lua`, `world/travel.lua`, and
`wizard/maintenance.lua`. Keep shared helpers in `../packages`.

Every command declaration must include `name`, `permission`, `pattern` and
`handler`. `permission` is `everyone`, `wizard` or `god`. Names are normalized
to lowercase command tokens without whitespace or `/`. Definitions are captured
at load time; changing the returned table does not update the registry.
Restricted commands are skipped so later eligible commands may match. Configured
command aliases rewrite the token to its canonical name before pattern matching.
