/* help_index.h - Index of markdown help articles under the configured root. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/help/help_types.h"
#include "mux/objects/db.h"

typedef struct HelpIndex HelpIndex;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerLog ServerLog;

/* Builds an independently owned index. Errors are logged to player. */
HelpIndex *help_index_create(EvaluationContext *evaluation, ServerLog *log,
                             const char *root_directory, DbRef player);
void help_index_destroy(HelpIndex *index);

/*
 * Rebuilds the index from scratch, discarding all prior state. Errors and a
 * summary are logged and, when player != NOTHING, also sent to the player.
 */
void help_index_reload(EvaluationContext *evaluation, HelpIndex *index,
                       DbRef player);

/* The game/help/index.md article, or nullptr if it wasn't indexed. */
const HelpArticle *help_index_default_article(const HelpIndex *index);

/*
 * Exact lookup by lowercased keyword. Returns nullptr on a miss, and also
 * treats a wizard_only article as a miss for a non-wizard viewer.
 */
const HelpArticle *help_index_find_exact(const HelpIndex *index,
                                         const char *keyword_lower,
                                         bool viewer_is_wizard);

size_t help_index_article_count(const HelpIndex *index);
const HelpArticle *help_index_article_at(const HelpIndex *index,
                                         size_t article_index);

/* The deduplicated keyword index (one entry per reachable keyword). */
size_t help_index_keyword_count(const HelpIndex *index);
const char *help_index_keyword_at(const HelpIndex *index, size_t keyword_index);
const HelpArticle *help_index_keyword_article_at(const HelpIndex *index,
                                                 size_t keyword_index);

/* Error/warning counts from the most recent index build, for testing. */
size_t help_index_last_error_count(const HelpIndex *index);
size_t help_index_last_warning_count(const HelpIndex *index);

/*
 * Re-reads the article's markdown body (everything after the frontmatter)
 * from disk. Returns a malloc'd, NUL-terminated buffer the caller must
 * free(), or nullptr on read failure.
 */
char *help_index_read_body(const HelpIndex *index, const HelpArticle *article,
                           size_t *out_length);
