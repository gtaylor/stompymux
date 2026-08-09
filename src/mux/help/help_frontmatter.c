/* help_frontmatter.c - TOML frontmatter parsing for help articles. */

#include "mux/help/help_frontmatter.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/help/help_types.h"
#include "mux/support/checked_storage.h"
#include "tomlc17.h"

static char **help_string_slot(HelpStringList *list, size_t index) {
  return checked_storage_at(list->items, list->count, sizeof(*list->items),
                            index);
}

static char *help_frontmatter_dup(const char *s) {
  size_t length;
  char *copy;

  length = strlen(s);
  copy = malloc(length + 1);
  memcpy(copy, s, length + 1);
  return copy;
}

static bool help_frontmatter_copy_string_list(toml_datum_t array,
                                              HelpStringList *out) {
  size_t i;

  if (array.type != TOML_ARRAY || array.u.arr.size <= 0)
    return false;
  out->count = (size_t)array.u.arr.size;
  out->items = malloc(out->count * sizeof(char *));
  for (i = 0; i < out->count; i++) {
    toml_datum_t element = *(const toml_datum_t *)checked_storage_at_const(
        array.u.arr.elem, out->count, sizeof(*array.u.arr.elem), i);

    if (element.type != TOML_STRING) {
      *help_string_slot(out, i) = help_frontmatter_dup("");
      continue;
    }
    *help_string_slot(out, i) = help_frontmatter_dup(element.u.s);
  }
  return true;
}

bool help_frontmatter_parse(const char *text, size_t length, HelpArticle *out,
                            char *error, size_t error_size) {
  toml_result_t result;
  toml_datum_t datum;
  bool ok = true;
  char *nul_terminated;

  error[0] = '\0';
  /* toml_parse requires text[length] == '\0'; `text` usually points into a
   * larger buffer (the article's frontmatter span), so make an isolated
   * NUL-terminated copy first. */
  nul_terminated = malloc(length + 1);
  memcpy(nul_terminated, text, length);
  *(char *)checked_storage_at(nul_terminated, length + 1, sizeof(char),
                              length) = '\0';
  result = toml_parse(nul_terminated, (int)length);
  free(nul_terminated);
  if (!result.ok) {
    (void)snprintf(error, error_size, "%s", result.errmsg);
    toml_free(result);
    return false;
  }

  datum = toml_get(result.toptab, "title");
  if (datum.type != TOML_STRING) {
    (void)snprintf(error, error_size,
                   "missing required frontmatter field 'title'");
    ok = false;
    goto done;
  }
  out->title = help_frontmatter_dup(datum.u.s);

  datum = toml_get(result.toptab, "description");
  if (datum.type != TOML_STRING) {
    (void)snprintf(error, error_size,
                   "missing required frontmatter field 'description'");
    ok = false;
    goto done;
  }
  out->description = help_frontmatter_dup(datum.u.s);

  datum = toml_get(result.toptab, "keywords");
  if (!help_frontmatter_copy_string_list(datum, &out->keywords)) {
    (void)snprintf(error, error_size,
                   "missing required frontmatter field 'keywords'");
    ok = false;
    goto done;
  }

  datum = toml_get(result.toptab, "article_tags");
  help_frontmatter_copy_string_list(datum, &out->article_tags);

  datum = toml_get(result.toptab, "show_index_for_article_tags");
  help_frontmatter_copy_string_list(datum, &out->show_index_for_article_tags);

  out->index_style = HELP_INDEX_STYLE_LIST_WITH_DESCRIPTION;
  datum = toml_get(result.toptab, "index_style");
  if (datum.type == TOML_STRING) {
    if (!strcmp(datum.u.s, "columnar"))
      out->index_style = HELP_INDEX_STYLE_COLUMNAR;
    else if (!strcmp(datum.u.s, "list_with_description"))
      out->index_style = HELP_INDEX_STYLE_LIST_WITH_DESCRIPTION;
    else
      (void)snprintf(error, error_size,
                     "unrecognized index_style '%s'; defaulting to "
                     "list_with_description",
                     datum.u.s);
  }

  datum = toml_get(result.toptab, "weight");
  if (datum.type == TOML_INT64) {
    out->has_weight = true;
    out->weight = (long)datum.u.int64;
  }

  datum = toml_get(result.toptab, "wizard_only");
  if (datum.type == TOML_BOOLEAN)
    out->wizard_only = datum.u.boolean;

done:
  toml_free(result);
  return ok;
}

void help_frontmatter_free(HelpArticle *article) {
  size_t i;

  free(article->title);
  free(article->description);
  for (i = 0; i < article->keywords.count; i++)
    free(*help_string_slot(&article->keywords, i));
  free(article->keywords.items);
  for (i = 0; i < article->article_tags.count; i++)
    free(*help_string_slot(&article->article_tags, i));
  free(article->article_tags.items);
  for (i = 0; i < article->show_index_for_article_tags.count; i++)
    free(*help_string_slot(&article->show_index_for_article_tags, i));
  free(article->show_index_for_article_tags.items);
  free(article->relative_path);
}
