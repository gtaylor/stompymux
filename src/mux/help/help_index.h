/* help_index.h - Index of markdown help articles under mudconf.help_dir. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/database/db.h"
#include "mux/help/help_types.h"

/* Builds the index at boot. Errors are logged only (player is NOTHING). */
void help_index_init(void);

/*
 * Rebuilds the index from scratch, discarding all prior state. Errors and a
 * summary are logged and, when player != NOTHING, also sent to the player.
 */
void help_index_reload(DbRef player);

/* The game/help/index.md article, or nullptr if it wasn't indexed. */
const HelpArticle *help_index_default_article(void);

/*
 * Exact lookup by lowercased keyword. Returns nullptr on a miss, and also
 * treats a wizard_only article as a miss for a non-wizard viewer.
 */
const HelpArticle *help_index_find_exact(const char *keyword_lower,
                                         bool viewer_is_wizard);

size_t help_index_article_count(void);
const HelpArticle *help_index_article_at(size_t index);

/* The deduplicated keyword index (one entry per reachable keyword). */
size_t help_index_keyword_count(void);
const char *help_index_keyword_at(size_t index);
const HelpArticle *help_index_keyword_article_at(size_t index);

/* Error/warning counts from the most recent index build, for testing. */
size_t help_index_last_error_count(void);
size_t help_index_last_warning_count(void);

/*
 * Re-reads the article's markdown body (everything after the frontmatter)
 * from disk. Returns a malloc'd, NUL-terminated buffer the caller must
 * free(), or nullptr on read failure.
 */
char *help_index_read_body(const HelpArticle *article, size_t *out_length);
