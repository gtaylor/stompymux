/** @file
 * Index of markdown help articles under the configured root.
 */
#pragma once

#include <stddef.h>

#include "mux/commands/command_context.h"
#include "mux/help/help_types.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"

typedef struct HelpIndex HelpIndex;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerLog ServerLog;

/* Builds an independently owned index. Errors are logged to player. */
/** Creates help index. @param[in] evaluation Expression evaluation context.
 * @param[in] log Server log. @param[in] root_directory Root directory.
 * @param[in] player Player object. */

HelpIndex *help_index_create(EvaluationContext *evaluation, ServerLog *log,
                             const char *root_directory, DbRef player);
/** Destroys help index. @param[in,out] index Zero-based index. */

void help_index_destroy(HelpIndex *index);

/*
 * Rebuilds the index from scratch, discarding all prior state. Errors and a
 * summary are logged and, when player != NOTHING, also sent to the player.
 */
/** Executes help index reload. @param[in,out] evaluation Expression evaluation
 * context. @param[in,out] index Zero-based index. @param[in] player Player
 * object. */

void help_index_reload(EvaluationContext *evaluation, HelpIndex *index,
                       DbRef player);

/* The game/help/index.md article, or nullptr if it wasn't indexed. */
/** Executes help index default article. @param[in] index Zero-based index. */

const HelpArticle *help_index_default_article(const HelpIndex *index);

/*
 * Exact lookup by lowercased keyword. Returns nullptr on a miss, and also
 * treats a wizard_only article as a miss for a non-wizard viewer.
 */
/** Finds help index find exact. @param[in] index Zero-based index. @param[in]
 * keyword_lower Keyword lower. @param[in] viewer_is_wizard Viewer is wizard. */

const HelpArticle *help_index_find_exact(const HelpIndex *index,
                                         const char *keyword_lower,
                                         bool viewer_is_wizard);

/** Counts help index article. @param[in] index Zero-based index. */

size_t help_index_article_count(const HelpIndex *index);
/** Returns help index article at. @param[in] index Zero-based index. @param[in]
 * article_index Article index. */

const HelpArticle *help_index_article_at(const HelpIndex *index,
                                         size_t article_index);

/* The deduplicated keyword index (one entry per reachable keyword). */
/** Counts help index keyword. @param[in] index Zero-based index. */

size_t help_index_keyword_count(const HelpIndex *index);
/** Returns help index keyword at. @param[in] index Zero-based index. @param[in]
 * keyword_index Keyword index. */

const char *help_index_keyword_at(const HelpIndex *index, size_t keyword_index);
/** Returns help index keyword article at. @param[in] index Zero-based index.
 * @param[in] keyword_index Keyword index. */

const HelpArticle *help_index_keyword_article_at(const HelpIndex *index,
                                                 size_t keyword_index);

/* Error/warning counts from the most recent index build, for testing. */
/** Counts help index last error. @param[in] index Zero-based index. */

size_t help_index_last_error_count(const HelpIndex *index);
/** Counts help index last warning. @param[in] index Zero-based index. */

size_t help_index_last_warning_count(const HelpIndex *index);

/*
 * Re-reads the article's markdown body (everything after the frontmatter)
 * from disk. Returns a malloc'd, NUL-terminated buffer the caller must
 * free(), or nullptr on read failure.
 */
/** Reads body for help index. @param[in] index Zero-based index. @param[in]
 * article Article. @param[out] out_length Out length. */

char *help_index_read_body(const HelpIndex *index, const HelpArticle *article,
                           size_t *out_length);
