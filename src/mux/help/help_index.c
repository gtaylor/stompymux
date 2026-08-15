/* help_index.c - Recursive indexing of markdown help articles. */

#include <ctype.h>
#include <dirent.h>
#include <fcntl.h> // IWYU pragma: keep
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "mux/help/help_frontmatter.h"
#include "mux/help/help_index.h"
#include "mux/help/help_types.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"

struct HelpIndex {
  ServerLog *log;
  HelpArticleVector articles;
  HelpKeywordEntry *keywords;
  size_t keyword_count;
  size_t default_article_index;
  size_t last_error_count;
  size_t last_warning_count;
  char *root_directory;
};

static HelpArticle *help_article_slot(HelpArticleVector *vector, size_t index) {
  return checked_storage_at(vector->items, vector->capacity,
                            sizeof(*vector->items), index);
}

static HelpArticle *help_article_item(HelpArticleVector *vector, size_t index) {
  return checked_storage_at(vector->items, vector->count,
                            sizeof(*vector->items), index);
}

static const HelpArticle *
help_article_item_const(const HelpArticleVector *vector, size_t index) {
  return checked_storage_at_const(vector->items, vector->count,
                                  sizeof(*vector->items), index);
}

static HelpKeywordEntry *help_keyword_slot(HelpIndex *index, size_t capacity,
                                           size_t position) {
  return checked_storage_at(index->keywords, capacity, sizeof(*index->keywords),
                            position);
}

static const HelpKeywordEntry *help_keyword_item(const HelpIndex *index,
                                                 size_t position) {
  return checked_storage_at_const(index->keywords, index->keyword_count,
                                  sizeof(*index->keywords), position);
}

static const char *help_string_item(const HelpStringList *list, size_t index) {
  return *(char *const *)checked_storage_at_const(
      (const void *)list->items, list->count, sizeof(*list->items), index);
}

static char **help_name_slot(char **names, size_t capacity, size_t index) {
  return (char **)checked_storage_at((void *)names, capacity, sizeof(*names),
                                     index);
}

static char *help_name_item(char *const *names, size_t count, size_t index) {
  return *(char *const *)checked_storage_at_const((const void *)names, count,
                                                  sizeof(*names), index);
}

static char help_character_at(const char *text, size_t length, size_t index) {
  return *(const char *)checked_storage_at_const(text, length + 1, sizeof(char),
                                                 index);
}

static char *help_join_path(const char *base, const char *name) {
  size_t base_length = strlen(base);
  size_t name_length = strlen(name);
  size_t capacity;
  char *joined;

  if (base_length > SIZE_MAX - name_length - 2)
    return nullptr;
  capacity = base_length + name_length + 2;
  joined = checked_storage_try_allocate(capacity);
  if (joined == nullptr)
    return nullptr;

  /* capacity accounts for both names, the separator, and the terminator. */
  (void)snprintf(joined, capacity, "%s/%s", base, name);
  return joined;
}

static int help_index_name_compare(const ArraySortComparison *comparison) {
  const char *left;
  const char *right;

  memcpy((void *)&left, comparison->left, sizeof(left));
  memcpy((void *)&right, comparison->right, sizeof(right));
  return strcmp(left, right);
}

static int help_index_keyword_compare(const ArraySortComparison *comparison) {
  HelpKeywordEntry left;
  HelpKeywordEntry right;

  memcpy(&left, comparison->left, sizeof(left));
  memcpy(&right, comparison->right, sizeof(right));
  return strcmp(left.keyword, right.keyword);
}

static void help_article_vector_push(HelpArticleVector *vector,
                                     const HelpArticle *article) {
  if (vector->count == vector->capacity) {
    size_t capacity = vector->capacity ? vector->capacity * 2 : 16;
    HelpArticle *items;

    if (capacity < vector->capacity || capacity > SIZE_MAX / sizeof(*items))
      abort();
    items = checked_storage_try_reallocate(vector->items,
                                           capacity * sizeof(*items));
    if (items == nullptr)
      abort();
    vector->items = items;
    vector->capacity = capacity;
  }
  *help_article_slot(vector, vector->count++) = *article;
}

static char *help_slurp_file(const char *path, size_t *out_length) {
  FILE *fp;
  long size;
  char *buffer;

  fp = fopen(path, "rb");
  if (!fp)
    return nullptr;
  bool read_failed = fseek(fp, 0, SEEK_END) != 0;
  if (!read_failed) {
    size = ftell(fp);
    read_failed = size < 0;
  }
  if (!read_failed)
    read_failed = fseek(fp, 0, SEEK_SET) != 0;
  if (read_failed) {
    if (fclose(fp) != 0)
      return nullptr;
    return nullptr;
  }
  buffer = checked_storage_try_allocate((size_t)size + 1);
  if (!buffer) {
    if (fclose(fp) != 0)
      return nullptr;
    return nullptr;
  }
  if (fread(buffer, 1, (size_t)size, fp) != (size_t)size) {
    free(buffer);
    if (fclose(fp) != 0)
      return nullptr;
    return nullptr;
  }
  if (fclose(fp) != 0) {
    free(buffer);
    return nullptr;
  }
  // File length is converted only after ftell() verifies it is non-negative.
  *(char *)checked_storage_at(buffer, (size_t)size + 1, sizeof(char),
                              (size_t)size) = '\0';
  if (out_length)
    *out_length = (size_t)size;
  return buffer;
}

/*
 * Splits `content` into a TOML frontmatter span and a body span, delimited
 * by lines that are exactly "+++". Returns false if the file doesn't start
 * with such a line, or the closing delimiter is never found.
 */
static bool help_locate_frontmatter(const char *content,
                                    const char **toml_start,
                                    size_t *toml_length,
                                    const char **body_start) {
  const size_t CONTENT_LENGTH = strlen(content);
  size_t line_end = 0;
  size_t toml_offset;
  size_t cursor;

  if (CONTENT_LENGTH < 3 || strncmp(content, "+++", 3) != 0)
    return false;
  while (line_end < CONTENT_LENGTH &&
         help_character_at(content, CONTENT_LENGTH, line_end) != '\n')
    line_end++;
  if (line_end == CONTENT_LENGTH)
    return false;
  for (size_t index = 3; index < line_end; index++) {
    const char CHARACTER = help_character_at(content, CONTENT_LENGTH, index);

    if (CHARACTER != '\r' && CHARACTER != ' ' && CHARACTER != '\t')
      return false;
  }
  toml_offset = line_end + 1;
  *toml_start = checked_string_suffix(content, toml_offset);

  cursor = toml_offset;
  for (;;) {
    size_t line_length;
    size_t trimmed_length;

    line_end = cursor;
    while (line_end < CONTENT_LENGTH &&
           help_character_at(content, CONTENT_LENGTH, line_end) != '\n')
      line_end++;
    line_length = line_end - cursor;
    trimmed_length = line_length;
    if (trimmed_length > 0 &&
        help_character_at(content, CONTENT_LENGTH,
                          cursor + trimmed_length - 1) == '\r')
      trimmed_length--;
    if (trimmed_length == 3 &&
        strncmp(checked_string_suffix(content, cursor), "+++", 3) == 0) {
      *toml_length = cursor - toml_offset;
      *body_start = checked_string_suffix(
          content, line_end < CONTENT_LENGTH ? line_end + 1 : line_end);
      return true;
    }
    if (line_end == CONTENT_LENGTH)
      return false;
    cursor = line_end + 1;
  }
}

typedef struct HelpFileProcessRequest {
  EvaluationContext *evaluation;
  HelpIndex *index;
  const char *absolute_path;
  const char *relative_path;
  DbRef player;
  int *error_count;
} HelpFileProcessRequest;

static void help_index_process_file(const HelpFileProcessRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  HelpIndex *index = request->index;
  const char *absolute_path = request->absolute_path;
  const char *relative_path = request->relative_path;
  DbRef player = request->player;
  int *error_count = request->error_count;
  char *content;
  const char *toml_start;
  size_t toml_length;
  const char *body_start;
  HelpArticle article;
  char error[256];

  content = help_slurp_file(absolute_path, nullptr);
  if (!content) {
    if (index->log != nullptr) {
      log_error((LogEntry){.log = index->log,
                           .key = LOG_PROBLEMS,
                           .primary = "HLP",
                           .secondary = "READ"},
                "%s: unable to read file", relative_path);
    }
    if (player != NOTHING)
      notify_printf(evaluation, player,
                    "Help index error: %s: unable to read file", relative_path);
    (*error_count)++;
    return;
  }
  if (!help_locate_frontmatter(content, &toml_start, &toml_length,
                               &body_start)) {
    if (index->log != nullptr) {
      log_error((LogEntry){.log = index->log,
                           .key = LOG_PROBLEMS,
                           .primary = "HLP",
                           .secondary = "PARSE"},
                "%s: missing +++ frontmatter delimiters", relative_path);
    }
    if (player != NOTHING)
      notify_printf(evaluation, player,
                    "Help index error: %s: missing +++ frontmatter "
                    "delimiters",
                    relative_path);
    (*error_count)++;
    free(content);
    return;
  }

  memset(&article, 0, sizeof(article));
  if (!help_frontmatter_parse(toml_start, toml_length, &article, error,
                              sizeof(error))) {
    if (index->log != nullptr) {
      log_error((LogEntry){.log = index->log,
                           .key = LOG_PROBLEMS,
                           .primary = "HLP",
                           .secondary = "PARSE"},
                "%s: %s", relative_path, error);
    }
    if (player != NOTHING)
      notify_printf(evaluation, player, "Help index error: %s: %s",
                    relative_path, error);
    (*error_count)++;
    help_frontmatter_free(&article);
    free(content);
    return;
  }
  if (error[0]) {
    if (index->log != nullptr) {
      log_error((LogEntry){.log = index->log,
                           .key = LOG_STARTUP,
                           .primary = "HLP",
                           .secondary = "WARN"},
                "%s: %s", relative_path, error);
    }
    if (player != NOTHING)
      notify_printf(evaluation, player, "Help index warning: %s: %s",
                    relative_path, error);
  }

  article.relative_path = strdup(relative_path);
  if (article.relative_path == nullptr) {
    (*error_count)++;
    help_frontmatter_free(&article);
    free(content);
    return;
  }
  help_article_vector_push(&index->articles, &article);
  free(content);
}

static void help_index_walk_directory(EvaluationContext *evaluation,
                                      HelpIndex *index,
                                      const char *absolute_dir,
                                      const char *relative_prefix, DbRef player,
                                      int *error_count) {
  DIR *stream;
  struct dirent *entry;
  char **entry_names = nullptr;
  size_t name_count = 0;
  size_t name_capacity = 0;
  size_t i;

  stream = opendir(absolute_dir);
  if (!stream) {
    if (index->log != nullptr) {
      log_error((LogEntry){.log = index->log,
                           .key = LOG_PROBLEMS,
                           .primary = "HLP",
                           .secondary = "OPENDIR"},
                "unable to open help directory '%s'", absolute_dir);
    }
    if (player != NOTHING)
      notify_printf(evaluation, player,
                    "Help index error: unable to open help directory '%s'",
                    absolute_dir);
    (*error_count)++;
    return;
  }
  while ((entry = readdir(stream))) {
    if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, ".."))
      continue;
    if (name_count == name_capacity) {
      size_t capacity = name_capacity ? name_capacity * 2 : 16;
      char **names;

      if (capacity < name_capacity || capacity > SIZE_MAX / sizeof(*names))
        abort();
      names = (char **)checked_storage_try_reallocate(
          (void *)entry_names, capacity * sizeof(*names));
      if (names == nullptr)
        abort();
      entry_names = names;
      name_capacity = capacity;
    }
    char *entry_name = strdup(entry->d_name);
    if (entry_name == nullptr)
      abort();
    *help_name_slot(entry_names, name_capacity, name_count++) = entry_name;
  }
  closedir(stream);
  if (name_count > 0)
    array_sort(&(ArraySortRequest){.items = (void *)entry_names,
                                   .count = name_count,
                                   .item_size = sizeof(*entry_names),
                                   .compare = help_index_name_compare});

  for (i = 0; i < name_count; i++) {
    char *entry_name = help_name_item(entry_names, name_count, i);
    char *absolute_child = help_join_path(absolute_dir, entry_name);
    char *relative_child = relative_prefix[0]
                               ? help_join_path(relative_prefix, entry_name)
                               : strdup(entry_name);
    struct stat status;

    if (absolute_child == nullptr || relative_child == nullptr) {
      free(absolute_child);
      free(relative_child);
      free(entry_name);
      (*error_count)++;
      continue;
    }
    if (stat(absolute_child, &status) == 0) {
      if (S_ISDIR(status.st_mode)) {
        help_index_walk_directory(evaluation, index, absolute_child,
                                  relative_child, player, error_count);
      } else if (S_ISREG(status.st_mode)) {
        size_t name_length = strlen(entry_name);

        if (name_length > 3 &&
            !strcmp(checked_string_suffix(entry_name, name_length - 3),
                    ".md")) {
          help_index_process_file(
              &(HelpFileProcessRequest){.evaluation = evaluation,
                                        .index = index,
                                        .absolute_path = absolute_child,
                                        .relative_path = relative_child,
                                        .player = player,
                                        .error_count = error_count});
        }
      }
    }
    free(absolute_child);
    free(relative_child);
    free(entry_name);
  }
  free((void *)entry_names);
}

static void help_index_build_keywords(EvaluationContext *evaluation,
                                      HelpIndex *index, DbRef player,
                                      int *warning_count) {
  size_t total_keywords = 0;
  size_t i;
  size_t k;

  for (i = 0; i < index->articles.count; i++)
    total_keywords += help_article_item(&index->articles, i)->keywords.count;
  if (total_keywords == 0)
    return;
  index->keywords =
      checked_storage_allocate(total_keywords * sizeof(HelpKeywordEntry));

  for (i = 0; i < index->articles.count; i++) {
    HelpArticle *article = help_article_item(&index->articles, i);

    for (k = 0; k < article->keywords.count; k++) {
      char *keyword_lower = strdup(help_string_item(&article->keywords, k));
      size_t keyword_length = strlen(keyword_lower);
      size_t existing;
      bool duplicate = false;

      for (size_t position = 0; position < keyword_length; position++) {
        char *character = checked_storage_at(keyword_lower, keyword_length + 1,
                                             sizeof(char), position);

        *character = (char)(tolower)((unsigned char)*character);
      }

      for (existing = 0; existing < index->keyword_count; existing++) {
        if (!strcmp(help_keyword_item(index, existing)->keyword,
                    keyword_lower)) {
          duplicate = true;
          break;
        }
      }
      if (duplicate) {
        HelpArticle *owner = help_article_item(
            &index->articles,
            help_keyword_item(index, existing)->article_index);

        if (index->log != nullptr) {
          log_error((LogEntry){.log = index->log,
                               .key = LOG_STARTUP,
                               .primary = "HLP",
                               .secondary = "DUPKW"},
                    "keyword '%s' declared by both '%s' and '%s'; '%s' wins",
                    keyword_lower, owner->relative_path, article->relative_path,
                    owner->relative_path);
        }
        if (player != NOTHING) {
          notify_printf(evaluation, player,
                        "Help index warning: keyword '%s' declared by both "
                        "'%s' and '%s'; '%s' wins",
                        keyword_lower, owner->relative_path,
                        article->relative_path, owner->relative_path);
        }
        (*warning_count)++;
        free(keyword_lower);
        continue;
      }
      HelpKeywordEntry *keyword =
          help_keyword_slot(index, total_keywords, index->keyword_count);

      keyword->keyword = keyword_lower;
      keyword->article_index = i;
      index->keyword_count++;
    }
  }
  array_sort(&(ArraySortRequest){.items = index->keywords,
                                 .count = index->keyword_count,
                                 .item_size = sizeof(*index->keywords),
                                 .compare = help_index_keyword_compare});
}

static void help_index_reset(HelpIndex *index) {
  size_t i;

  for (i = 0; i < index->articles.count; i++)
    help_frontmatter_free(help_article_item(&index->articles, i));
  free(index->articles.items);
  index->articles = (HelpArticleVector){0};

  for (i = 0; i < index->keyword_count; i++)
    free(help_keyword_item(index, i)->keyword);
  free(index->keywords);
  index->keywords = nullptr;
  index->keyword_count = 0;

  index->default_article_index = SIZE_MAX;
}

static void help_index_rebuild(EvaluationContext *evaluation, HelpIndex *index,
                               DbRef player) {
  int error_count = 0;
  int warning_count = 0;
  size_t i;

  help_index_reset(index);
  help_index_walk_directory(evaluation, index, index->root_directory, "",
                            player, &error_count);
  help_index_build_keywords(evaluation, index, player, &warning_count);

  for (i = 0; i < index->articles.count; i++) {
    if (!strcmp(help_article_item(&index->articles, i)->relative_path,
                "index.md")) {
      index->default_article_index = i;
      break;
    }
  }

  index->last_error_count = (size_t)error_count;
  index->last_warning_count = (size_t)warning_count;

  if (index->log != nullptr) {
    log_error((LogEntry){.log = index->log,
                         .key = LOG_STARTUP,
                         .primary = "HLP",
                         .secondary = "IDX"},
              "Indexed %zu article(s), %zu keyword(s), %d error(s), %d "
              "warning(s)",
              index->articles.count, index->keyword_count, error_count,
              warning_count);
  }
  if (player != NOTHING) {
    notify_printf(evaluation, player,
                  "Help reindexed: %zu article(s), %zu keyword(s), %d "
                  "error(s), %d warning(s).",
                  index->articles.count, index->keyword_count, error_count,
                  warning_count);
  }
}

HelpIndex *help_index_create(EvaluationContext *evaluation, ServerLog *log,
                             const char *root_directory, DbRef player) {
  HelpIndex *index = checked_storage_try_allocate_array(1, sizeof(*index));

  if (index == nullptr)
    return nullptr;
  index->log = log;
  index->root_directory = strdup(root_directory);
  if (index->root_directory == nullptr) {
    free(index);
    return nullptr;
  }
  index->default_article_index = SIZE_MAX;
  help_index_rebuild(evaluation, index, player);
  return index;
}

void help_index_destroy(HelpIndex *index) {
  if (index == nullptr)
    return;
  help_index_reset(index);
  free(index->root_directory);
  free(index);
}

void help_index_reload(EvaluationContext *evaluation, HelpIndex *index,
                       DbRef player) {
  help_index_rebuild(evaluation, index, player);
}

const HelpArticle *help_index_default_article(const HelpIndex *index) {
  if (index->default_article_index == SIZE_MAX)
    return nullptr;
  return help_article_item_const(&index->articles,
                                 index->default_article_index);
}

const HelpArticle *help_index_find_exact(const HelpIndex *index,
                                         const char *keyword_lower,
                                         bool viewer_is_wizard) {
  size_t low = 0;
  size_t high = index->keyword_count;

  while (low < high) {
    size_t mid = low + ((high - low) / 2);
    const HelpKeywordEntry *keyword = help_keyword_item(index, mid);
    int comparison = strcmp(keyword->keyword, keyword_lower);

    if (comparison == 0) {
      const HelpArticle *article =
          help_article_item_const(&index->articles, keyword->article_index);

      if (article->wizard_only && !viewer_is_wizard)
        return nullptr;
      return article;
    }
    if (comparison < 0)
      low = mid + 1;
    else
      high = mid;
  }
  return nullptr;
}

size_t help_index_article_count(const HelpIndex *index) {
  return index->articles.count;
}

const HelpArticle *help_index_article_at(const HelpIndex *index,
                                         size_t article_index) {
  return help_article_item_const(&index->articles, article_index);
}

size_t help_index_last_error_count(const HelpIndex *index) {
  return index->last_error_count;
}

size_t help_index_last_warning_count(const HelpIndex *index) {
  return index->last_warning_count;
}

size_t help_index_keyword_count(const HelpIndex *index) {
  return index->keyword_count;
}

const char *help_index_keyword_at(const HelpIndex *index,
                                  size_t keyword_index) {
  return help_keyword_item(index, keyword_index)->keyword;
}

const HelpArticle *help_index_keyword_article_at(const HelpIndex *index,
                                                 size_t keyword_index) {
  return help_article_item_const(
      &index->articles, help_keyword_item(index, keyword_index)->article_index);
}

char *help_index_read_body(const HelpIndex *index, const HelpArticle *article,
                           size_t *out_length) {
  char *absolute_path =
      help_join_path(index->root_directory, article->relative_path);
  char *content;
  const char *toml_start;
  size_t toml_length;
  const char *body_start;
  size_t body_length;
  char *body;

  content = help_slurp_file(absolute_path, nullptr);
  free(absolute_path);
  if (!content)
    return nullptr;
  if (!help_locate_frontmatter(content, &toml_start, &toml_length,
                               &body_start)) {
    free(content);
    return nullptr;
  }
  body_length = strlen(body_start);
  body = checked_storage_allocate(body_length + 1);
  memcpy(body, body_start, body_length + 1);
  free(content);
  if (out_length)
    *out_length = body_length;
  return body;
}
