# Global Lua logic

Put globally matched command and scheduled-logic modules here. Files are
discovered recursively and run in lexical relative-path order. A command
handler returning `true` stops global dispatch; `false` or `nil` allows the
next handler to try.

Use domain-oriented paths such as `player/help.lua`, `world/travel.lua`, and
`wizard/maintenance.lua`. Keep shared helpers in `../packages`.
