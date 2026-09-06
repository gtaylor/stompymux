-- Explicit legacy styling and immutable Markdown documents.
local native, mux, id = ...
mux.text = {
    markdown = native.markdown,
    is_printable_ascii = native.printable_ascii,
    markup = native.markup,
    style = native.style,
    width = native.width,
    truncate = native.truncate,
    strip_style = native.strip,
}
