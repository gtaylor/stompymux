/** @file
 * TOML frontmatter parsing for help articles.
 */
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
/** Parses help frontmatter. @param[in] text Text to process. @param[in] length
 * Text or storage length. @param[out] out Out. @param[out] error Storage
 * receiving an error description. @param[in] error_size Size of error in bytes.
 */

bool help_frontmatter_parse(const char *text, size_t length, HelpArticle *out,
                            char *error, size_t error_size);

/** Releases help frontmatter. @param[in,out] article Article. */

void help_frontmatter_free(HelpArticle *article);
