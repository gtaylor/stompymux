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
  static const char *const chan_keywords[] = {
      "@chan/boot",  "@chan/create", "@chan/destroy", "@chan/emit",
      "@chan/list",  "@chan/object", "@chan/oflags",  "@chan/pflags",
      "@chan/flags", "@chan/status", "@chan/who",
  };
  static const char *const lua_keywords[] = {
      "@lua/check",
      "@lua/parent",
      "@lua/reload",
      "@lua/schedule",
  };
  static const char *const removed_lua_keywords[] = {
      "@luacheck",
      "@luaparent",
      "@luareload",
      "@luaschedule",
  };
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

  article = help_index_find_exact(index, "@chan", false);
  if (article) {
    fprintf(stderr, "expected '@chan' help to be hidden from a non-wizard\n");
    return 7;
  }
  article = help_index_find_exact(index, "@chan", true);
  if (!article || strcmp(article->relative_path, "wizard_commands/chan.md") ||
      article->show_index_for_article_tags.count != 1 ||
      strcmp(article->show_index_for_article_tags.items[0], "chan_switches")) {
    fprintf(stderr, "expected '@chan' to resolve to its switch index\n");
    return 7;
  }

  for (size_t i = 0; i < sizeof(chan_keywords) / sizeof(chan_keywords[0]);
       i++) {
    article = help_index_find_exact(index, chan_keywords[i], false);
    if (article) {
      fprintf(stderr, "expected '%s' help to be hidden from a non-wizard\n",
              chan_keywords[i]);
      return 8;
    }
    article = help_index_find_exact(index, chan_keywords[i], true);
    if (!article || !article->wizard_only || article->article_tags.count != 1 ||
        strcmp(article->article_tags.items[0], "chan_switches")) {
      fprintf(stderr, "expected '%s' to resolve to a @chan switch article\n",
              chan_keywords[i]);
      return 8;
    }
  }
  if (help_index_find_exact(index, "@chan/chown", true)) {
    fprintf(stderr, "expected removed '@chan/chown' help to be absent\n");
    return 8;
  }

  article = help_index_find_exact(index, "@help", false);
  if (article) {
    fprintf(stderr, "expected '@help' help to be hidden from a non-wizard\n");
    return 8;
  }
  article = help_index_find_exact(index, "@help", true);
  if (!article || strcmp(article->relative_path, "wizard_commands/help.md") ||
      article->show_index_for_article_tags.count != 1 ||
      strcmp(article->show_index_for_article_tags.items[0], "help_switches")) {
    fprintf(stderr, "expected '@help' to resolve to its switch index\n");
    return 8;
  }
  article = help_index_find_exact(index, "@help/reload", false);
  if (article) {
    fprintf(stderr,
            "expected '@help/reload' help to be hidden from a non-wizard\n");
    return 8;
  }
  article = help_index_find_exact(index, "@help/reload", true);
  if (!article || !article->wizard_only || article->article_tags.count != 1 ||
      strcmp(article->article_tags.items[0], "help_switches")) {
    fprintf(stderr, "expected '@help/reload' to resolve to a switch article\n");
    return 8;
  }
  if (help_index_find_exact(index, "@helpreload", true)) {
    fprintf(stderr, "expected removed '@helpreload' help to be absent\n");
    return 8;
  }

  article = help_index_find_exact(index, "@lua", false);
  if (article) {
    fprintf(stderr, "expected '@lua' help to be hidden from a non-wizard\n");
    return 8;
  }
  article = help_index_find_exact(index, "@lua", true);
  if (!article || strcmp(article->relative_path, "wizard_commands/lua.md") ||
      article->show_index_for_article_tags.count != 1 ||
      strcmp(article->show_index_for_article_tags.items[0], "lua_switches")) {
    fprintf(stderr, "expected '@lua' to resolve to its switch index\n");
    return 8;
  }
  for (size_t i = 0; i < sizeof(lua_keywords) / sizeof(lua_keywords[0]); i++) {
    article = help_index_find_exact(index, lua_keywords[i], false);
    if (article) {
      fprintf(stderr, "expected '%s' help to be hidden from a non-wizard\n",
              lua_keywords[i]);
      return 8;
    }
    article = help_index_find_exact(index, lua_keywords[i], true);
    if (!article || !article->wizard_only || article->article_tags.count != 1 ||
        strcmp(article->article_tags.items[0], "lua_switches")) {
      fprintf(stderr, "expected '%s' to resolve to a @lua switch article\n",
              lua_keywords[i]);
      return 8;
    }
  }
  for (size_t i = 0;
       i < sizeof(removed_lua_keywords) / sizeof(removed_lua_keywords[0]);
       i++) {
    if (help_index_find_exact(index, removed_lua_keywords[i], true)) {
      fprintf(stderr, "expected removed '%s' help to be absent\n",
              removed_lua_keywords[i]);
      return 8;
    }
  }

  /* Nested article, reachable via its own keyword. */
  article = help_index_find_exact(index, "another", false);
  if (!article || strcmp(article->relative_path, "subdir/another_article.md")) {
    fprintf(stderr, "expected 'another' keyword to resolve to "
                    "subdir/another_article.md\n");
    return 9;
  }
  if (article->article_tags.count != 1 ||
      strcmp(article->article_tags.items[0], "subdir")) {
    fprintf(stderr, "expected subdir/another_article.md to be tagged "
                    "'subdir'\n");
    return 10;
  }

  /* index.md is the default article. */
  default_article = help_index_default_article(index);
  if (!default_article || strcmp(default_article->relative_path, "index.md")) {
    fprintf(stderr, "expected index.md to be the default article\n");
    return 11;
  }

  second_index = help_index_create(nullptr, nullptr, argv[1], NOTHING);
  if (second_index == nullptr)
    return 12;
  help_index_destroy(index);
  article = help_index_find_exact(second_index, "about", false);
  if (!article || strcmp(article->relative_path, "about.md")) {
    fprintf(stderr, "destroying one help index changed another instance\n");
    return 12;
  }
  help_index_destroy(second_index);
  return 0;
}
