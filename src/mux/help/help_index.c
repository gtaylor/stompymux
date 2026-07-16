/* help_index.c - Recursive indexing of markdown help articles. */

#include "mux/server/platform.h"

#include <ctype.h>
#include <dirent.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "mux/help/help_frontmatter.h"
#include "mux/help/help_index.h"
#include "mux/help/help_types.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/server_state.h"

static HelpArticleVector help_articles;
static HelpKeywordEntry *help_keywords;
static size_t help_keyword_count;
static size_t help_default_article_index = SIZE_MAX;
static size_t help_last_error_count;
static size_t help_last_warning_count;

static char *help_join_path(const char *base, const char *name) {
  size_t base_length = strlen(base);
  size_t name_length = strlen(name);
  char *joined = malloc(base_length + 1 + name_length + 1);

  memcpy(joined, base, base_length);
  joined[base_length] = '/';
  memcpy(joined + base_length + 1, name, name_length + 1);
  return joined;
}

static int help_index_name_compare(const void *a, const void *b) {
  return strcmp(*(char *const *)a, *(char *const *)b);
}

static int help_index_keyword_compare(const void *a, const void *b) {
  return strcmp(((const HelpKeywordEntry *)a)->keyword,
                ((const HelpKeywordEntry *)b)->keyword);
}

static void help_article_vector_push(HelpArticleVector *vector,
                                     const HelpArticle *article) {
  if (vector->count == vector->capacity) {
    vector->capacity = vector->capacity ? vector->capacity * 2 : 16;
    vector->items =
        realloc(vector->items, vector->capacity * sizeof(HelpArticle));
  }
  vector->items[vector->count++] = *article;
}

static char *help_slurp_file(const char *path, size_t *out_length) {
  FILE *fp;
  long size;
  char *buffer;

  fp = fopen(path, "rb");
  if (!fp)
    return nullptr;
  if (fseek(fp, 0, SEEK_END) != 0 || (size = ftell(fp)) < 0 ||
      fseek(fp, 0, SEEK_SET) != 0) {
    fclose(fp);
    return nullptr;
  }
  buffer = malloc((size_t)size + 1);
  if (fread(buffer, 1, (size_t)size, fp) != (size_t)size) {
    free(buffer);
    fclose(fp);
    return nullptr;
  }
  fclose(fp);
  buffer[size] = '\0';
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
  const char *cursor;
  const char *line_end;
  const char *p;

  if (strncmp(content, "+++", 3) != 0)
    return false;
  line_end = strchr(content, '\n');
  if (!line_end)
    return false;
  for (p = content + 3; p < line_end; p++) {
    if (*p != '\r' && *p != ' ' && *p != '\t')
      return false;
  }
  *toml_start = line_end + 1;

  cursor = *toml_start;
  for (;;) {
    size_t line_length;
    size_t trimmed_length;

    line_end = strchr(cursor, '\n');
    line_length = line_end ? (size_t)(line_end - cursor) : strlen(cursor);
    trimmed_length = line_length;
    if (trimmed_length > 0 && cursor[trimmed_length - 1] == '\r')
      trimmed_length--;
    if (trimmed_length == 3 && strncmp(cursor, "+++", 3) == 0) {
      *toml_length = (size_t)(cursor - *toml_start);
      *body_start = line_end ? line_end + 1 : cursor + line_length;
      return true;
    }
    if (!line_end)
      return false;
    cursor = line_end + 1;
  }
}

static void help_index_process_file(const char *absolute_path,
                                    const char *relative_path, DbRef player,
                                    int *error_count) {
  char *content;
  const char *toml_start;
  size_t toml_length;
  const char *body_start;
  HelpArticle article;
  char error[256];

  content = help_slurp_file(absolute_path, nullptr);
  if (!content) {
    log_error(LOG_PROBLEMS, "HLP", "READ", "%s: unable to read file",
              relative_path);
    if (player != NOTHING)
      notify_printf(player, "Help index error: %s: unable to read file",
                    relative_path);
    (*error_count)++;
    return;
  }
  if (!help_locate_frontmatter(content, &toml_start, &toml_length,
                               &body_start)) {
    log_error(LOG_PROBLEMS, "HLP", "PARSE",
              "%s: missing +++ frontmatter delimiters", relative_path);
    if (player != NOTHING)
      notify_printf(player,
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
    log_error(LOG_PROBLEMS, "HLP", "PARSE", "%s: %s", relative_path, error);
    if (player != NOTHING)
      notify_printf(player, "Help index error: %s: %s", relative_path, error);
    (*error_count)++;
    help_frontmatter_free(&article);
    free(content);
    return;
  }
  if (error[0]) {
    log_error(LOG_STARTUP, "HLP", "WARN", "%s: %s", relative_path, error);
    if (player != NOTHING)
      notify_printf(player, "Help index warning: %s: %s", relative_path, error);
  }

  article.relative_path = strdup(relative_path);
  help_article_vector_push(&help_articles, &article);
  free(content);
}

static void help_index_walk_directory(const char *absolute_dir,
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
    log_error(LOG_PROBLEMS, "HLP", "OPENDIR",
              "unable to open help directory '%s'", absolute_dir);
    if (player != NOTHING)
      notify_printf(player,
                    "Help index error: unable to open help directory '%s'",
                    absolute_dir);
    (*error_count)++;
    return;
  }
  while ((entry = readdir(stream))) {
    if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, ".."))
      continue;
    if (name_count == name_capacity) {
      name_capacity = name_capacity ? name_capacity * 2 : 16;
      entry_names = realloc(entry_names, name_capacity * sizeof(char *));
    }
    entry_names[name_count++] = strdup(entry->d_name);
  }
  closedir(stream);
  if (name_count > 0)
    qsort(entry_names, name_count, sizeof(char *), help_index_name_compare);

  for (i = 0; i < name_count; i++) {
    char *absolute_child = help_join_path(absolute_dir, entry_names[i]);
    char *relative_child = relative_prefix[0]
                               ? help_join_path(relative_prefix, entry_names[i])
                               : strdup(entry_names[i]);
    struct stat status;

    if (stat(absolute_child, &status) == 0) {
      if (S_ISDIR(status.st_mode)) {
        help_index_walk_directory(absolute_child, relative_child, player,
                                  error_count);
      } else if (S_ISREG(status.st_mode)) {
        size_t name_length = strlen(entry_names[i]);

        if (name_length > 3 && !strcmp(entry_names[i] + name_length - 3, ".md"))
          help_index_process_file(absolute_child, relative_child, player,
                                  error_count);
      }
    }
    free(absolute_child);
    free(relative_child);
    free(entry_names[i]);
  }
  free(entry_names);
}

static void help_index_build_keywords(DbRef player, int *warning_count) {
  size_t total_keywords = 0;
  size_t i, k;

  for (i = 0; i < help_articles.count; i++)
    total_keywords += help_articles.items[i].keywords.count;
  if (total_keywords == 0)
    return;
  help_keywords = malloc(total_keywords * sizeof(HelpKeywordEntry));

  for (i = 0; i < help_articles.count; i++) {
    HelpArticle *article = &help_articles.items[i];

    for (k = 0; k < article->keywords.count; k++) {
      char *keyword_lower = strdup(article->keywords.items[k]);
      char *p;
      size_t existing;
      bool duplicate = false;

      for (p = keyword_lower; *p; p++)
        *p = (char)tolower((unsigned char)*p);

      for (existing = 0; existing < help_keyword_count; existing++) {
        if (!strcmp(help_keywords[existing].keyword, keyword_lower)) {
          duplicate = true;
          break;
        }
      }
      if (duplicate) {
        HelpArticle *owner =
            &help_articles.items[help_keywords[existing].article_index];

        log_error(LOG_STARTUP, "HLP", "DUPKW",
                  "keyword '%s' declared by both '%s' and '%s'; '%s' wins",
                  keyword_lower, owner->relative_path, article->relative_path,
                  owner->relative_path);
        if (player != NOTHING)
          notify_printf(player,
                        "Help index warning: keyword '%s' declared by both "
                        "'%s' and '%s'; '%s' wins",
                        keyword_lower, owner->relative_path,
                        article->relative_path, owner->relative_path);
        (*warning_count)++;
        free(keyword_lower);
        continue;
      }
      help_keywords[help_keyword_count].keyword = keyword_lower;
      help_keywords[help_keyword_count].article_index = i;
      help_keyword_count++;
    }
  }
  qsort(help_keywords, help_keyword_count, sizeof(HelpKeywordEntry),
        help_index_keyword_compare);
}

static void help_index_reset(void) {
  size_t i;

  for (i = 0; i < help_articles.count; i++)
    help_frontmatter_free(&help_articles.items[i]);
  free(help_articles.items);
  help_articles.items = nullptr;
  help_articles.count = 0;
  help_articles.capacity = 0;

  for (i = 0; i < help_keyword_count; i++)
    free(help_keywords[i].keyword);
  free(help_keywords);
  help_keywords = nullptr;
  help_keyword_count = 0;

  help_default_article_index = SIZE_MAX;
}

static void help_index_rebuild(DbRef player) {
  int error_count = 0;
  int warning_count = 0;
  size_t i;

  help_index_reset();
  help_index_walk_directory(mudconf.help_dir, "", player, &error_count);
  help_index_build_keywords(player, &warning_count);

  for (i = 0; i < help_articles.count; i++) {
    if (!strcmp(help_articles.items[i].relative_path, "index.md")) {
      help_default_article_index = i;
      break;
    }
  }

  help_last_error_count = (size_t)error_count;
  help_last_warning_count = (size_t)warning_count;

  log_error(LOG_STARTUP, "HLP", "IDX",
            "Indexed %zu article(s), %zu keyword(s), %d error(s), %d "
            "warning(s)",
            help_articles.count, help_keyword_count, error_count,
            warning_count);
  if (player != NOTHING)
    notify_printf(player,
                  "Help reindexed: %zu article(s), %zu keyword(s), %d "
                  "error(s), %d warning(s).",
                  help_articles.count, help_keyword_count, error_count,
                  warning_count);
}

void help_index_init(void) { help_index_rebuild(NOTHING); }

void help_index_reload(DbRef player) { help_index_rebuild(player); }

const HelpArticle *help_index_default_article(void) {
  if (help_default_article_index == SIZE_MAX)
    return nullptr;
  return &help_articles.items[help_default_article_index];
}

const HelpArticle *help_index_find_exact(const char *keyword_lower,
                                         bool viewer_is_wizard) {
  size_t low = 0;
  size_t high = help_keyword_count;

  while (low < high) {
    size_t mid = low + (high - low) / 2;
    int comparison = strcmp(help_keywords[mid].keyword, keyword_lower);

    if (comparison == 0) {
      const HelpArticle *article =
          &help_articles.items[help_keywords[mid].article_index];

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

size_t help_index_article_count(void) { return help_articles.count; }

const HelpArticle *help_index_article_at(size_t index) {
  return &help_articles.items[index];
}

size_t help_index_last_error_count(void) { return help_last_error_count; }

size_t help_index_last_warning_count(void) { return help_last_warning_count; }

size_t help_index_keyword_count(void) { return help_keyword_count; }

const char *help_index_keyword_at(size_t index) {
  return help_keywords[index].keyword;
}

const HelpArticle *help_index_keyword_article_at(size_t index) {
  return &help_articles.items[help_keywords[index].article_index];
}

char *help_index_read_body(const HelpArticle *article, size_t *out_length) {
  char *absolute_path =
      help_join_path(mudconf.help_dir, article->relative_path);
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
  body = malloc(body_length + 1);
  memcpy(body, body_start, body_length + 1);
  free(content);
  if (out_length)
    *out_length = body_length;
  return body;
}
