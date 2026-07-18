/* help_index_integration.c -- Indexes the real game/help fixtures and checks
 * wizard_only filtering, nested indexing, and the default article, per the
 * behavior confirmed while planning the help rewrite. */

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "mux/database/db.h"
#include "mux/help/help_index.h"
#include "mux/help/help_types.h"
#include "mux/server/log.h"

void log_error(ServerLog *log, int key, const char *primary,
               const char *secondary, const char *format, ...) {
  (void)log;
  (void)key;
  (void)primary;
  (void)secondary;
  (void)format;
}

void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...) {
  (void)evaluation;
  (void)player;
  (void)format;
}

static int help_index_test_find_by_path(const HelpIndex *index,
                                        const char *relative_path,
                                        const HelpArticle **out) {
  size_t i;

  for (i = 0; i < help_index_article_count(index); i++) {
    const HelpArticle *article = help_index_article_at(index, i);

    if (!strcmp(article->relative_path, relative_path)) {
      *out = article;
      return 1;
    }
  }
  return 0;
}

int main(int argc, char *argv[]) {
  const HelpArticle *article;
  const HelpArticle *wizards_article;
  const HelpArticle *default_article;
  HelpIndex *index;
  HelpIndex *second_index;

  if (argc != 2) {
    fprintf(stderr, "usage: %s <help-directory>\n", argv[0]);
    return 1;
  }
  index = help_index_create(nullptr, nullptr, argv[1], NOTHING);
  if (index == nullptr)
    return 2;

  if (help_index_last_warning_count(index) != 0) {
    fprintf(stderr, "expected no duplicate-keyword warnings\n");
    return 2;
  }

  /* about.md is public and owns the 'about' keyword. */
  article = help_index_find_exact(index, "about", false);
  if (!article || strcmp(article->relative_path, "about.md")) {
    fprintf(stderr, "expected 'about' keyword to resolve to about.md\n");
    return 3;
  }

  /* wizards.md is wizard_only: a non-wizard's exact match is treated as a
   * miss, but a wizard's resolves to the article. */
  article = help_index_find_exact(index, "wizards", false);
  if (article) {
    fprintf(stderr,
           "expected 'wizards' keyword to be hidden from a non-wizard\n");
    return 4;
  }
  article = help_index_find_exact(index, "wizards", true);
  if (!article || strcmp(article->relative_path, "wizards.md")) {
    fprintf(stderr, "expected 'wizards' keyword to resolve to wizards.md "
                   "for a wizard\n");
    return 4;
  }

  /* wizards.md itself is still indexed regardless of viewer. */
  if (!help_index_test_find_by_path(index, "wizards.md", &wizards_article)) {
    fprintf(stderr, "expected wizards.md to still be indexed\n");
    return 5;
  }
  if (!wizards_article->wizard_only) {
    fprintf(stderr, "expected wizards.md to be wizard_only\n");
    return 6;
  }

  /* Nested article, reachable via its own keyword. */
  article = help_index_find_exact(index, "another", false);
  if (!article || strcmp(article->relative_path, "subdir/another_article.md")) {
    fprintf(stderr, "expected 'another' keyword to resolve to "
                   "subdir/another_article.md\n");
    return 7;
  }
  if (article->article_tags.count != 1 ||
      strcmp(article->article_tags.items[0], "subdir")) {
    fprintf(stderr, "expected subdir/another_article.md to be tagged "
                   "'subdir'\n");
    return 8;
  }

  /* index.md is the default article. */
  default_article = help_index_default_article(index);
  if (!default_article || strcmp(default_article->relative_path, "index.md")) {
    fprintf(stderr, "expected index.md to be the default article\n");
    return 9;
  }

  second_index = help_index_create(nullptr, nullptr, argv[1], NOTHING);
  if (second_index == nullptr)
    return 10;
  help_index_destroy(index);
  article = help_index_find_exact(second_index, "about", false);
  if (!article || strcmp(article->relative_path, "about.md")) {
    fprintf(stderr, "destroying one help index changed another instance\n");
    return 10;
  }
  help_index_destroy(second_index);
  return 0;
}
