/* help_frontmatter_parse.c -- TOML help frontmatter parsing unit test */

#include <string.h>

#include "mux/help/help_frontmatter.h"
#include "mux/help/help_types.h"

static int help_frontmatter_test_full_success(void) {
  static const char toml[] = "title = \"About\"\n"
                            "description = \"All about this game\"\n"
                            "keywords = [\"wizards\", \"about\"]\n"
                            "article_tags = [\"show_in_index\"]\n"
                            "show_index_for_article_tags = [\"subdir\"]\n"
                            "index_style = \"columnar\"\n"
                            "weight = 3\n"
                            "wizard_only = true\n";
  HelpArticle article;
  char error[256];
  int ok;

  memset(&article, 0, sizeof(article));
  ok = help_frontmatter_parse(toml, sizeof(toml) - 1, &article, error,
                              sizeof(error));
  if (!ok || error[0])
    return 0;
  ok = ok && !strcmp(article.title, "About") &&
       !strcmp(article.description, "All about this game") &&
       article.keywords.count == 2 &&
       !strcmp(article.keywords.items[0], "wizards") &&
       !strcmp(article.keywords.items[1], "about") &&
       article.article_tags.count == 1 &&
       !strcmp(article.article_tags.items[0], "show_in_index") &&
       article.show_index_for_article_tags.count == 1 &&
       !strcmp(article.show_index_for_article_tags.items[0], "subdir") &&
       article.index_style == HELP_INDEX_STYLE_COLUMNAR &&
       article.has_weight && article.weight == 3 && article.wizard_only;
  help_frontmatter_free(&article);
  return ok;
}

static int help_frontmatter_test_missing_field(const char *toml) {
  HelpArticle article;
  char error[256];
  int ok;

  memset(&article, 0, sizeof(article));
  ok = help_frontmatter_parse(toml, strlen(toml), &article, error,
                              sizeof(error));
  help_frontmatter_free(&article);
  return !ok && error[0];
}

static int help_frontmatter_test_optional_defaults(void) {
  static const char toml[] = "title = \"About\"\n"
                            "description = \"desc\"\n"
                            "keywords = [\"about\"]\n";
  HelpArticle article;
  char error[256];
  int ok;

  memset(&article, 0, sizeof(article));
  ok = help_frontmatter_parse(toml, sizeof(toml) - 1, &article, error,
                              sizeof(error));
  ok = ok && !error[0] && !article.has_weight && !article.wizard_only &&
       article.index_style == HELP_INDEX_STYLE_LIST_WITH_DESCRIPTION &&
       article.article_tags.count == 0 &&
       article.show_index_for_article_tags.count == 0;
  help_frontmatter_free(&article);
  return ok;
}

static int help_frontmatter_test_unrecognized_index_style(void) {
  static const char toml[] = "title = \"About\"\n"
                            "description = \"desc\"\n"
                            "keywords = [\"about\"]\n"
                            "index_style = \"grid\"\n";
  HelpArticle article;
  char error[256];
  int ok;

  memset(&article, 0, sizeof(article));
  ok = help_frontmatter_parse(toml, sizeof(toml) - 1, &article, error,
                              sizeof(error));
  ok = ok && error[0] &&
       article.index_style == HELP_INDEX_STYLE_LIST_WITH_DESCRIPTION;
  help_frontmatter_free(&article);
  return ok;
}

static int help_frontmatter_test_malformed_toml(void) {
  static const char toml[] = "title = \n";
  HelpArticle article;
  char error[256];
  int ok;

  memset(&article, 0, sizeof(article));
  ok = help_frontmatter_parse(toml, sizeof(toml) - 1, &article, error,
                              sizeof(error));
  help_frontmatter_free(&article);
  return !ok && error[0];
}

int main(void) {
  if (!help_frontmatter_test_full_success())
    return 1;
  if (!help_frontmatter_test_missing_field(
          "description = \"desc\"\nkeywords = [\"a\"]\n"))
    return 2;
  if (!help_frontmatter_test_missing_field(
          "title = \"t\"\nkeywords = [\"a\"]\n"))
    return 3;
  if (!help_frontmatter_test_missing_field("title = \"t\"\ndescription = \"d\"\n"))
    return 4;
  if (!help_frontmatter_test_optional_defaults())
    return 5;
  if (!help_frontmatter_test_unrecognized_index_style())
    return 6;
  if (!help_frontmatter_test_malformed_toml())
    return 7;
  return 0;
}
