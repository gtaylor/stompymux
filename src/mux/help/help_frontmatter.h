/* help_frontmatter.h - TOML frontmatter parsing for help articles. */

#pragma once

#include <stddef.h>

#include "mux/help/help_types.h"

/*
 * Parses the TOML frontmatter block `text` (length `length`, need not be
 * NUL-terminated) into `out`. `out` must be zero-initialized by the caller;
 * `out->relative_path` is not touched here.
 *
 * Returns false only when a required field (title/description/keywords) is
 * missing or malformed TOML was given; `error` is filled with the reason.
 *
 * Returns true otherwise, including when an optional field had a bad value
 * (e.g. an unrecognized index_style) - in that case `error` is filled with a
 * non-fatal warning the caller may choose to log, otherwise `error[0]` is
 * '\0'.
 */
bool help_frontmatter_parse(const char *text, size_t length, HelpArticle *out,
                            char *error, size_t error_size);

void help_frontmatter_free(HelpArticle *article);
