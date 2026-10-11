+++
title = "MXP inline images"
description = "Show pictures inline in MUD clients that support MXP"
keywords = ["mxp", "images", "inline images", "image markup"]
article_tags = ["show_in_index"]
+++

# MXP inline images

Styled text can place a picture in the output with the same scoped syntax used
for colors and links. The text inside the tag is the fallback shown by clients
that cannot display the picture, so it is required.

```text
[image="https://cdn.example.com/mechs/atlas.png"]An Atlas AS7-D[/]
[image="https://cdn.example.com/maps/sector7.png" width=320 height=200 align=right]Sector 7 map[/]
```

The source is a quoted `http:`, `https:`, or `ftp:` URL that ends in a file
name and carries no query string or fragment. Optional `width` and `height`
are pixel sizes from 1 through 4096, and `align` is one of `left`, `right`,
`top`, `middle`, or `bottom`. Formatting tags may appear inside the fallback
text, but images cannot be nested in links or in other images, and links
cannot be nested in images. Markdown documents use the ordinary
`![alt text](URL)` form; an empty alt text falls back to the file name.

## What each client sees

The server offers the MUD eXtension Protocol (MXP) as Telnet option 91 to
every connection. A client that accepts it is locked into MXP's literal
text mode, so ordinary game text is never mistaken for markup, and is then
asked for its `<VERSION>` and `<SUPPORT>` replies. Only clients whose
`<SUPPORTS>` reply lists `+image` receive pictures; each one is sent as a
one-shot secure `<IMAGE>` tag naming the file and its download location.
Mudlet, zMUD, and CMUD are among the clients that support MXP images.

Clients without MXP image support see the fallback text. When such a client
advertises OSC 8 web links (see `help osc8`), the fallback text becomes a
link to the picture; in Markdown documents without link support the URL is
appended in parentheses.

Wizards can confirm what a connection reported with `@telnet <player>`, which
lists the client name, its MXP version, and the tags it advertised. Lua code
can call `mux.telnet.mxp_client(descriptor)` and
`mux.telnet.mxp_supports(descriptor, "image")` to tailor output.

Player-authored say, page, and ordinary channel messages continue to remove
all styled-text markup before delivery, so only builders and scripts can place
images.
