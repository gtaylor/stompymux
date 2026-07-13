# Global Lua commands

Put globally matched command modules here. Files are discovered recursively and
run in lexical relative-path order. A handler returning `true` stops global
dispatch; `false` or `nil` allows the next handler to try.

Use domain-oriented paths such as `player/help.lua`, `world/travel.lua`, and
`wizard/maintenance.lua`. Keep shared helpers in `../packages`.