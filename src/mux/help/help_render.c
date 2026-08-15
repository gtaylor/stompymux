/* help_render.c - Renders help articles to plain text for display. */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "cmark.h"
#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"

static char *help_buffer_slot(HelpTextBuffer *buffer, size_t index) {
  return checked_storage_at(buffer->data, buffer->capacity, sizeof(char),
                            index);
}

static char help_buffer_character(const HelpTextBuffer *buffer, size_t index) {
  return *(const char *)checked_storage_at_const(buffer->data, buffer->capacity,
                                                 sizeof(char), index);
}

static char help_text_character(const char *text, size_t length, size_t index) {
  return *(const char *)checked_storage_at_const(text, length + 1, sizeof(char),
                                                 index);
}

static const char *help_text_suffix(const char *text, size_t length,
                                    size_t offset) {
  return checked_storage_at_const(text, length + 1, sizeof(char), offset);
}

static const char *help_string_list_item(const HelpStringList *list,
                                         size_t index) {
  return *(char *const *)checked_storage_at_const(
      (const void *)list->items, list->count, sizeof(*list->items), index);
}

static const HelpArticle **help_article_slot(const HelpArticle **articles,
                                             size_t capacity, size_t index) {
  return (const HelpArticle **)checked_storage_at((void *)articles, capacity,
                                                  sizeof(*articles), index);
}

static const HelpArticle *help_article_item(const HelpArticle *const *articles,
                                            size_t count, size_t index) {
  return *(const HelpArticle *const *)checked_storage_at_const(
      (const void *)articles, count, sizeof(*articles), index);
}

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
  size_t required;

  if (length > SIZE_MAX - buffer->length - 1)
    abort();
  required = buffer->length + length + 1;
  if (required > buffer->capacity) {
    size_t capacity = buffer->capacity ? buffer->capacity : 256;

    while (capacity < required) {
      if (capacity > SIZE_MAX / 2) {
        capacity = required;
        break;
      }
      capacity *= 2;
    }
    char *data = checked_storage_try_reallocate(buffer->data, capacity);

    if (data == nullptr)
      abort();
    buffer->data = data;
    buffer->capacity = capacity;
  }
  if (length > 0)
    memcpy(checked_storage_region(buffer->data, buffer->capacity,
                                  buffer->length, length),
           checked_storage_region_const(text, length, 0, length), length);
  buffer->length += length;
  *help_buffer_slot(buffer, buffer->length) = '\0';
}

static void help_text_buffer_append_str(HelpTextBuffer *buffer,
                                        const char *text) {
  help_text_buffer_append(buffer, text, strlen(text));
}

static void help_text_buffer_append_code(HelpTextBuffer *buffer,
                                         const char *text) {
  const size_t LENGTH = strlen(text);

  for (size_t index = 0; index < LENGTH; index++) {
    if (help_text_character(text, LENGTH, index) == '[')
      help_text_buffer_append_str(buffer, "[");
    help_text_buffer_append(buffer, help_text_suffix(text, LENGTH, index), 1);
  }
}

static bool help_url_is_external(const char *url) {
  size_t body_offset;
  size_t length;

  if (url == nullptr)
    return false;
  length = strlen(url);
  if (!strncasecmp(url, "http:", 5))
    body_offset = 5;
  else if (!strncasecmp(url, "https:", 6))
    body_offset = 6;
  else if (!strncasecmp(url, "ftp:", 4))
    body_offset = 4;
  else
    return false;
  if (body_offset >= length || length > 4096)
    return false;
  for (size_t index = 0; index < length; index++) {
    unsigned char byte = (unsigned char)help_text_character(url, length, index);
    bool unreserved =
        (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z') ||
        (byte >= '0' && byte <= '9') || strchr("-._~", byte) != nullptr;

    if (byte == '%' && index + 2 < length &&
        (isxdigit)((unsigned char)help_text_character(url, length,
                                                      index + 1)) &&
        (isxdigit)((unsigned char)help_text_character(url, length,
                                                      index + 2))) {
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
  const size_t LENGTH = strlen(text);

  for (size_t index = 0; index < LENGTH; index++) {
    const char CHARACTER = help_text_character(text, LENGTH, index);

    if (CHARACTER == '\\' || CHARACTER == '"')
      help_text_buffer_append_str(buffer, "\\");
    help_text_buffer_append(buffer, help_text_suffix(text, LENGTH, index), 1);
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
  if (buffer->length >= 2 &&
      help_buffer_character(buffer, buffer->length - 1) == '\n' &&
      help_buffer_character(buffer, buffer->length - 2) == '\n')
    return;
  if (help_buffer_character(buffer, buffer->length - 1) != '\n')
    help_text_buffer_append_str(buffer, "\n");
  help_text_buffer_append_str(buffer, "\n");
}

static void help_render_ensure_newline(HelpTextBuffer *buffer) {
  if (buffer->length == 0)
    return;
  if (help_buffer_character(buffer, buffer->length - 1) != '\n')
    help_text_buffer_append_str(buffer, "\n");
}

static bool help_article_matches_tags(const HelpArticle *article,
                                      const HelpStringList *tags) {
  size_t i;
  size_t j;

  for (i = 0; i < article->article_tags.count; i++)
    for (j = 0; j < tags->count; j++)
      if (!strcmp(help_string_list_item(&article->article_tags, i),
                  help_string_list_item(tags, j)))
        return true;
  return false;
}

static int help_index_entry_compare(const ArraySortComparison *comparison) {
  const HelpArticle *left = *(const HelpArticle *const *)comparison->left;
  const HelpArticle *right = *(const HelpArticle *const *)comparison->right;

  if (left->has_weight && right->has_weight) {
    if (left->weight != right->weight)
      return left->weight < right->weight ? -1 : 1;
    return strcasecmp(help_string_list_item(&left->keywords, 0),
                      help_string_list_item(&right->keywords, 0));
  }
  if (left->has_weight != right->has_weight)
    return left->has_weight ? -1 : 1;
  return strcasecmp(help_string_list_item(&left->article_tags, 0),
                    help_string_list_item(&right->article_tags, 0));
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
  entries = (const HelpArticle **)checked_storage_allocate(
      total * sizeof(const HelpArticle *));
  for (i = 0; i < total; i++) {
    const HelpArticle *candidate = help_index_article_at(index, i);

    if (candidate == index_article)
      continue;
    if (candidate->wizard_only && !viewer_is_wizard)
      continue;
    if (!help_article_matches_tags(candidate,
                                   &index_article->show_index_for_article_tags))
      continue;
    *help_article_slot(entries, total, count++) = candidate;
  }
  array_sort(&(ArraySortRequest){.items = (void *)entries,
                                 .count = count,
                                 .item_size = sizeof(*entries),
                                 .compare = help_index_entry_compare});

  help_render_ensure_blank_line(out);
  if (index_article->index_style == HELP_INDEX_STYLE_COLUMNAR) {
    for (i = 0; i < count; i++) {
      const HelpArticle *entry = help_article_item(entries, count, i);
      const char *topic = help_string_list_item(&entry->keywords, 0);
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

      (void)snprintf(header, sizeof(header), "%-20s %s\n", "TOPIC",
                     "DESCRIPTION");
      help_text_buffer_append_str(out, header);
    }
    for (i = 0; i < count; i++) {
      const HelpArticle *entry = help_article_item(entries, count, i);
      const char *topic = help_string_list_item(&entry->keywords, 0);
      size_t topic_length = strlen(topic);

      help_text_buffer_append_help_link(out, topic);
      for (size_t padding = topic_length; padding < 20; padding++)
        help_text_buffer_append_str(out, " ");
      help_text_buffer_append_str(out, " ");
      help_text_buffer_append_str(out, entry->description);
      help_text_buffer_append_str(out, "\n");
    }
  }
  free((void *)entries);
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
  size_t offset = 0;

  if (buffer->data == nullptr)
    return;
  while (offset < buffer->length &&
         help_buffer_character(buffer, offset) != '\0') {
    size_t source_length = 0;
    char line[LBUF_SIZE];

    while (offset + source_length < buffer->length &&
           help_buffer_character(buffer, offset + source_length) != '\0' &&
           help_buffer_character(buffer, offset + source_length) != '\n')
      source_length++;
    size_t line_length = source_length;

    if (line_length >= sizeof(line))
      line_length = sizeof(line) - 1;
    if (source_length == 0) {
      /* notify_checked() silently drops empty messages, so a blank line needs a
       * single space to actually reach the player. */
      notify_checked(evaluation, player, player, " ", MSG_ME_ALL | MSG_F_DOWN);
      if (help_buffer_character(buffer, offset) != '\n')
        break;
      offset++;
      continue;
    }
    memcpy(line,
           checked_storage_region_const(buffer->data, buffer->capacity, offset,
                                        line_length),
           line_length);
    *(char *)checked_storage_at(line, sizeof(line), sizeof(char), line_length) =
        '\0';
    notify_checked(evaluation, player, player, line, MSG_ME_ALL | MSG_F_DOWN);
    offset += source_length;
    if (offset >= buffer->length ||
        help_buffer_character(buffer, offset) != '\n')
      break;
    offset++;
  }
}
