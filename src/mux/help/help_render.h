/** @file
 * Renders help articles to plain text for display.
 */
#pragma once

#include "mux/commands/command_runtime.h"
#include <stddef.h>

#include "mux/help/help_index.h"
#include "mux/help/help_types.h"
#include "mux/server/platform.h"

typedef struct HelpIndex HelpIndex;

typedef struct HelpTextBuffer {
  char *data;
  size_t length;
  size_t capacity;
} HelpTextBuffer;

/** Executes help text buffer init. @param[out] buffer Caller-owned output
 * storage. */

void help_text_buffer_init(HelpTextBuffer *buffer);
/** Releases help text buffer. @param[in,out] buffer Caller-owned output
 * storage. */

void help_text_buffer_free(HelpTextBuffer *buffer);

/*
 * Renders CommonMark `markdown` (headers as literal '#' lines, external links
 * as OSC-capable styled-text markup, and emphasis markers stripped down to
 * their visible text) into `out`. Inline and block code escapes styled-text
 * markup so the output boundary displays it literally. Does not touch the
 * help index; used both by
 * help_article_render_body and directly by unit tests.
 */
/** Executes help render markdown. @param[in] markdown Markdown. @param[in]
 * length Text or storage length. @param[out] out Out. */

void help_render_markdown(const char *markdown, size_t length,
                          HelpTextBuffer *out);

/*
 * Renders the article's markdown body via help_render_markdown. If the
 * article declares show_index_for_article_tags, an index section listing
 * matching articles is appended, with wizard_only articles omitted unless
 * viewer_is_wizard is true.
 */
/** Executes help article render body. @param[in] index Zero-based index.
 * @param[in] article Article. @param[in] viewer_is_wizard Viewer is wizard.
 * @param[out] out Out. */

void help_article_render_body(const HelpIndex *index,
                              const HelpArticle *article, bool viewer_is_wizard,
                              HelpTextBuffer *out);

/* Splits buffer->data on '\n' and calls notify_checked() once per line. */
/** Executes help render send. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] buffer Caller-owned
 * output storage. */

void help_render_send(EvaluationContext *evaluation, DbRef player,
                      const HelpTextBuffer *buffer);
