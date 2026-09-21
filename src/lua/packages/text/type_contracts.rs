//! LuaLS contract blocks for the mux text surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin mux 00135
//|---Tests whether every byte is printable ASCII (0x20 through 0x7e).
//|---@param value string
//|---@return boolean printable
//|function mux_text.is_printable_ascii(value) end
// lua-types-end

// lua-types-begin mux 00136
//|---Validates styled-text markup and returns it unchanged.
//|---@param value string
//|---@return string markup
//|---
//|---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
//|---@see mux.error.codes.text.invalid
//|function mux_text.markup(value) end
// lua-types-end

// lua-types-begin mux 00137
//|---Removes styled-text markup and ANSI styling.
//|---@param value string
//|---@return string plain
//|function mux_text.strip_style(value) end
// lua-types-end

// lua-types-begin mux 00138
//|---Wraps text in markup described by the supplied style options.
//|---@param value string
//|---@param options StyleOptions
//|---@return string styled
//|---
//|---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
//|---@see mux.error.codes.text.invalid
//|function mux_text.style(value, options) end
// lua-types-end

// lua-types-begin mux 00139
//|---Safely truncates styled text to a non-negative visible byte width.
//|---@param value string
//|---@param width integer
//|---@return string truncated
//|---
//|---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid).
//|---@see mux.error.codes.text.invalid
//|function mux_text.truncate(value, width) end
// lua-types-end

// lua-types-begin mux 00140
//|---Measures visible byte width, excluding markup and ANSI styling.
//|---@param value string
//|---@return integer width
//|function mux_text.width(value) end
// lua-types-end

// lua-types-begin mux 00148
//|---Immutable Markdown document retained by the Lua runtime.
//|---@class MarkdownDocument
//|
//|---Parses Markdown into an immutable document for text output.
//|---@param source string Markdown source.
//|---@return MarkdownDocument document
//|---
//|---Raises [`mux.error.codes.text.invalid`](lua://mux.error.codes.text.invalid) for invalid or oversized input.
//|---@see mux.error.codes.text.invalid
//|function mux_text.markdown(source) end
// lua-types-end
