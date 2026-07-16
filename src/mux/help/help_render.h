/* help_render.h - Renders help articles to plain text for display. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/database/db.h"
#include "mux/help/help_types.h"

typedef struct HelpTextBuffer {
  char *data;
  size_t length;
  size_t capacity;
} HelpTextBuffer;

void help_text_buffer_init(HelpTextBuffer *buffer);
void help_text_buffer_free(HelpTextBuffer *buffer);

/*
 * Renders CommonMark `markdown` (headers as literal '#' lines, links and
 * emphasis markers stripped down to their visible text) into `out`. Does not
 * touch the help index; used both by help_article_render_body and directly
 * by unit tests.
 */
void help_render_markdown(const char *markdown, size_t length,
                          HelpTextBuffer *out);

/*
 * Renders the article's markdown body via help_render_markdown. If the
 * article declares show_index_for_article_tags, an index section listing
 * matching articles is appended, with wizard_only articles omitted unless
 * viewer_is_wizard is true.
 */
void help_article_render_body(const HelpArticle *article, bool viewer_is_wizard,
                              HelpTextBuffer *out);

/* Splits buffer->data on '\n' and calls notify() once per line. */
void help_render_send(DbRef player, const HelpTextBuffer *buffer);
