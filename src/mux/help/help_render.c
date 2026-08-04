/* help_render.c - Renders help articles to plain text for display. */

#include "mux/server/game.h"
#include "mux/server/platform.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "cmark.h"

#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"

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

static void help_text_buffer_append_code(HelpTextBuffer *buffer,
                                         const char *text) {
  for (const char *cursor = text; *cursor; cursor++) {
    if (*cursor == '[')
      help_text_buffer_append_str(buffer, "[");
    help_text_buffer_append(buffer, cursor, 1);
  }
}

static bool help_url_is_external(const char *url) {
  const char *body;
  size_t length;

  if (url == nullptr)
    return false;
  if (!strncasecmp(url, "http:", 5))
    body = url + 5;
  else if (!strncasecmp(url, "https:", 6))
    body = url + 6;
  else if (!strncasecmp(url, "ftp:", 4))
    body = url + 4;
  else
    return false;
  length = strlen(url);
  if (*body == '\0' || length > 4096)
    return false;
  for (size_t index = 0; index < length; index++) {
    unsigned char byte = (unsigned char)url[index];
    bool unreserved =
        (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z') ||
        (byte >= '0' && byte <= '9') || strchr("-._~", byte) != nullptr;

    if (byte == '%' && index + 2 < length &&
        isxdigit((unsigned char)url[index + 1]) &&
        isxdigit((unsigned char)url[index + 2])) {
      index += 2;
      continue;
    }
    if (!unreserved && strchr(":/?#[]@!$&'()*+,;=", byte) == nullptr)
      return false;
  }
  return true;
}

static void help_text_buffer_append_quoted(HelpTextBuffer *buffer,
                                           const char *text) {
  for (const char *cursor = text; *cursor; cursor++) {
    if (*cursor == '\\' || *cursor == '"')
      help_text_buffer_append_str(buffer, "\\");
    help_text_buffer_append(buffer, cursor, 1);
  }
}

static void help_text_buffer_append_help_link(HelpTextBuffer *buffer,
                                              const char *topic) {
  help_text_buffer_append_str(buffer, "[send=\"help ");
  help_text_buffer_append_quoted(buffer, topic);
  help_text_buffer_append_str(buffer, "\"]");
  help_text_buffer_append_code(buffer, topic);
  help_text_buffer_append_str(buffer, "[/]");
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

static void help_render_index_section(const HelpIndex *index,
                                      const HelpArticle *index_article,
                                      bool viewer_is_wizard,
                                      HelpTextBuffer *out) {
  const HelpArticle **entries;
  size_t count = 0;
  size_t total = help_index_article_count(index);
  size_t i;

  if (total == 0)
    return;
  entries = malloc(total * sizeof(const HelpArticle *));
  for (i = 0; i < total; i++) {
    const HelpArticle *candidate = help_index_article_at(index, i);

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
      const char *topic = entries[i]->keywords.items[0];
      size_t topic_length = strlen(topic);

      help_text_buffer_append_help_link(out, topic);
      for (size_t padding = topic_length; padding < 20; padding++)
        help_text_buffer_append_str(out, " ");
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
      const char *topic = entries[i]->keywords.items[0];
      size_t topic_length = strlen(topic);

      help_text_buffer_append_help_link(out, topic);
      for (size_t padding = topic_length; padding < 20; padding++)
        help_text_buffer_append_str(out, " ");
      help_text_buffer_append_str(out, " ");
      help_text_buffer_append_str(out, entries[i]->description);
      help_text_buffer_append_str(out, "\n");
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
        help_text_buffer_append_code(out, cmark_node_get_literal(node));
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
      help_text_buffer_append_str(out, cmark_node_get_literal(node));
      break;
    case CMARK_NODE_CODE:
      help_text_buffer_append_code(out, cmark_node_get_literal(node));
      break;
    case CMARK_NODE_SOFTBREAK:
      help_text_buffer_append_str(out, " ");
      break;
    case CMARK_NODE_LINEBREAK:
      help_text_buffer_append_str(out, "\n");
      break;
    case CMARK_NODE_LINK: {
      const char *url = cmark_node_get_url(node);

      if (help_url_is_external(url)) {
        if (event == CMARK_EVENT_ENTER) {
          help_text_buffer_append_str(out, "[link=\"");
          help_text_buffer_append_quoted(out, url);
          help_text_buffer_append_str(out, "\"]");
        } else {
          help_text_buffer_append_str(out, "[/]");
        }
      }
      break;
    }
    case CMARK_NODE_NONE:
    case CMARK_NODE_DOCUMENT:
    case CMARK_NODE_HTML_BLOCK:
    case CMARK_NODE_CUSTOM_BLOCK:
    case CMARK_NODE_HTML_INLINE:
    case CMARK_NODE_CUSTOM_INLINE:
    case CMARK_NODE_EMPH:
    case CMARK_NODE_STRONG:
    case CMARK_NODE_IMAGE:
      break;
    }
  }
  cmark_iter_free(iter);
  cmark_node_free(root);
}

void help_article_render_body(const HelpIndex *index,
                              const HelpArticle *article, bool viewer_is_wizard,
                              HelpTextBuffer *out) {
  char *body;

  body = help_index_read_body(index, article, nullptr);
  if (!body) {
    help_text_buffer_append_str(out, "Unable to render article.");
    return;
  }
  help_render_markdown(body, strlen(body), out);
  free(body);

  if (article->show_index_for_article_tags.count > 0)
    help_render_index_section(index, article, viewer_is_wizard, out);
}

void help_render_send(EvaluationContext *evaluation, DbRef player,
                      const HelpTextBuffer *buffer) {
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
      /* notify_checked() silently drops empty messages, so a blank line needs a
       * single space to actually reach the player. */
      notify_checked(evaluation, player, player, " ", MSG_ME_ALL | MSG_F_DOWN);
      if (!line_end)
        break;
      cursor = line_end + 1;
      continue;
    }
    memcpy(line, cursor, line_length);
    line[line_length] = '\0';
    notify_checked(evaluation, player, player, line, MSG_ME_ALL | MSG_F_DOWN);
    if (!line_end)
      break;
    cursor = line_end + 1;
  }
}
