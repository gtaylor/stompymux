/* help_render.c - Renders help articles to plain text for display. */

#include "mux/server/platform.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "cmark.h"

#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/server_api.h"

void help_text_buffer_init(HelpTextBuffer *buffer) {
  buffer->data = nullptr;
  buffer->length = 0;
  buffer->capacity = 0;
}

void help_text_buffer_free(HelpTextBuffer *buffer) {
  free(buffer->data);
  buffer->data = nullptr;
  buffer->length = 0;
  buffer->capacity = 0;
}

static void help_text_buffer_append(HelpTextBuffer *buffer, const char *text,
                                    size_t length) {
  if (buffer->length + length + 1 > buffer->capacity) {
    buffer->capacity = (buffer->capacity ? buffer->capacity * 2 : 256);
    while (buffer->capacity < buffer->length + length + 1)
      buffer->capacity *= 2;
    buffer->data = realloc(buffer->data, buffer->capacity);
  }
  memcpy(buffer->data + buffer->length, text, length);
  buffer->length += length;
  buffer->data[buffer->length] = '\0';
}

static void help_text_buffer_append_str(HelpTextBuffer *buffer,
                                        const char *text) {
  help_text_buffer_append(buffer, text, strlen(text));
}

static void help_render_ensure_blank_line(HelpTextBuffer *buffer) {
  if (buffer->length == 0)
    return;
  if (buffer->length >= 2 && buffer->data[buffer->length - 1] == '\n' &&
      buffer->data[buffer->length - 2] == '\n')
    return;
  if (buffer->data[buffer->length - 1] != '\n')
    help_text_buffer_append_str(buffer, "\n");
  help_text_buffer_append_str(buffer, "\n");
}

static void help_render_ensure_newline(HelpTextBuffer *buffer) {
  if (buffer->length == 0)
    return;
  if (buffer->data[buffer->length - 1] != '\n')
    help_text_buffer_append_str(buffer, "\n");
}

static bool help_article_matches_tags(const HelpArticle *article,
                                      const HelpStringList *tags) {
  size_t i, j;

  for (i = 0; i < article->article_tags.count; i++)
    for (j = 0; j < tags->count; j++)
      if (!strcmp(article->article_tags.items[i], tags->items[j]))
        return true;
  return false;
}

static int help_index_entry_compare(const void *a, const void *b) {
  const HelpArticle *left = *(const HelpArticle *const *)a;
  const HelpArticle *right = *(const HelpArticle *const *)b;

  if (left->has_weight && right->has_weight) {
    if (left->weight != right->weight)
      return left->weight < right->weight ? -1 : 1;
    return strcasecmp(left->keywords.items[0], right->keywords.items[0]);
  }
  if (left->has_weight != right->has_weight)
    return left->has_weight ? -1 : 1;
  return strcasecmp(left->article_tags.items[0], right->article_tags.items[0]);
}

static void help_render_index_section(const HelpArticle *index_article,
                                      bool viewer_is_wizard,
                                      HelpTextBuffer *out) {
  const HelpArticle **entries;
  size_t count = 0;
  size_t total = help_index_article_count();
  size_t i;

  if (total == 0)
    return;
  entries = malloc(total * sizeof(const HelpArticle *));
  for (i = 0; i < total; i++) {
    const HelpArticle *candidate = help_index_article_at(i);

    if (candidate == index_article)
      continue;
    if (candidate->wizard_only && !viewer_is_wizard)
      continue;
    if (!help_article_matches_tags(candidate,
                                   &index_article->show_index_for_article_tags))
      continue;
    entries[count++] = candidate;
  }
  qsort(entries, count, sizeof(const HelpArticle *), help_index_entry_compare);

  help_render_ensure_blank_line(out);
  if (index_article->index_style == HELP_INDEX_STYLE_COLUMNAR) {
    for (i = 0; i < count; i++) {
      char column[32];

      snprintf(column, sizeof(column), "%-20s", entries[i]->keywords.items[0]);
      help_text_buffer_append_str(out, column);
      if ((i + 1) % 3 == 0)
        help_text_buffer_append_str(out, "\n");
    }
    if (count % 3 != 0)
      help_text_buffer_append_str(out, "\n");
  } else {
    if (count > 0) {
      char header[256];

      snprintf(header, sizeof(header), "%-20s %s\n", "TOPIC", "DESCRIPTION");
      help_text_buffer_append_str(out, header);
    }
    for (i = 0; i < count; i++) {
      char line[256];

      snprintf(line, sizeof(line), "%-20s %s\n", entries[i]->keywords.items[0],
               entries[i]->description);
      help_text_buffer_append_str(out, line);
    }
  }
  free(entries);
}

void help_render_markdown(const char *markdown, size_t length,
                          HelpTextBuffer *out) {
  cmark_node *root;
  cmark_iter *iter;
  cmark_event_type event;

  root = cmark_parse_document(markdown, length, CMARK_OPT_DEFAULT);

  iter = cmark_iter_new(root);
  while ((event = cmark_iter_next(iter)) != CMARK_EVENT_DONE) {
    cmark_node *node = cmark_iter_get_node(iter);
    cmark_node_type type = cmark_node_get_type(node);

    switch (type) {
    case CMARK_NODE_HEADING:
      if (event == CMARK_EVENT_ENTER) {
        int level = cmark_node_get_heading_level(node);
        int i;

        help_render_ensure_blank_line(out);
        for (i = 0; i < level; i++)
          help_text_buffer_append_str(out, "#");
        help_text_buffer_append_str(out, " ");
      }
      break;
    case CMARK_NODE_PARAGRAPH:
      if (event == CMARK_EVENT_ENTER &&
          cmark_node_get_type(cmark_node_parent(node)) != CMARK_NODE_ITEM)
        help_render_ensure_blank_line(out);
      break;
    case CMARK_NODE_BLOCK_QUOTE:
    case CMARK_NODE_THEMATIC_BREAK:
      if (event == CMARK_EVENT_ENTER)
        help_render_ensure_blank_line(out);
      break;
    case CMARK_NODE_CODE_BLOCK:
      if (event == CMARK_EVENT_ENTER) {
        help_render_ensure_blank_line(out);
        help_text_buffer_append_str(out, cmark_node_get_literal(node));
      }
      break;
    case CMARK_NODE_LIST:
      if (event == CMARK_EVENT_ENTER)
        help_render_ensure_blank_line(out);
      break;
    case CMARK_NODE_ITEM:
      if (event == CMARK_EVENT_ENTER) {
        help_render_ensure_newline(out);
        help_text_buffer_append_str(out, "- ");
      }
      break;
    case CMARK_NODE_TEXT:
    case CMARK_NODE_CODE:
      help_text_buffer_append_str(out, cmark_node_get_literal(node));
      break;
    case CMARK_NODE_SOFTBREAK:
      help_text_buffer_append_str(out, " ");
      break;
    case CMARK_NODE_LINEBREAK:
      help_text_buffer_append_str(out, "\n");
      break;
    case CMARK_NODE_NONE:
    case CMARK_NODE_DOCUMENT:
    case CMARK_NODE_HTML_BLOCK:
    case CMARK_NODE_CUSTOM_BLOCK:
    case CMARK_NODE_HTML_INLINE:
    case CMARK_NODE_CUSTOM_INLINE:
    case CMARK_NODE_EMPH:
    case CMARK_NODE_STRONG:
    case CMARK_NODE_LINK:
    case CMARK_NODE_IMAGE:
      break;
    }
  }
  cmark_iter_free(iter);
  cmark_node_free(root);
}

void help_article_render_body(const HelpArticle *article, bool viewer_is_wizard,
                              HelpTextBuffer *out) {
  char *body;

  body = help_index_read_body(article, nullptr);
  if (!body) {
    help_text_buffer_append_str(out, "Unable to render article.");
    return;
  }
  help_render_markdown(body, strlen(body), out);
  free(body);

  if (article->show_index_for_article_tags.count > 0)
    help_render_index_section(article, viewer_is_wizard, out);
}

void help_render_send(DbRef player, const HelpTextBuffer *buffer) {
  const char *cursor = buffer->data;

  if (!cursor)
    return;
  while (*cursor) {
    const char *line_end = strchr(cursor, '\n');
    size_t line_length =
        line_end ? (size_t)(line_end - cursor) : strlen(cursor);
    char line[LBUF_SIZE];

    if (line_length >= sizeof(line))
      line_length = sizeof(line) - 1;
    if (line_length == 0) {
      /* notify() silently drops empty messages, so a blank line needs a
       * single space to actually reach the player. */
      notify(player, " ");
      if (!line_end)
        break;
      cursor = line_end + 1;
      continue;
    }
    memcpy(line, cursor, line_length);
    line[line_length] = '\0';
    notify(player, line);
    if (!line_end)
      break;
    cursor = line_end + 1;
  }
}
