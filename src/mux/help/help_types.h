/* help_types.h - Shared data structures for the markdown help system. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef enum HelpIndexStyle {
  HELP_INDEX_STYLE_LIST_WITH_DESCRIPTION,
  HELP_INDEX_STYLE_COLUMNAR,
} HelpIndexStyle;

typedef struct HelpStringList {
  char **items;
  size_t count;
} HelpStringList;

typedef struct HelpArticle {
  char *relative_path;     /* path under help_dir, forward-slash separated */
  char *title;             /* required */
  char *description;       /* required */
  HelpStringList keywords; /* required, count >= 1 */
  HelpStringList article_tags;                /* optional */
  HelpStringList show_index_for_article_tags; /* optional; non-empty means
                                                  this article renders in
                                                  index mode */
  HelpIndexStyle index_style;
  bool has_weight;
  long weight;
  bool wizard_only;
} HelpArticle;

typedef struct HelpArticleVector {
  HelpArticle *items;
  size_t count;
  size_t capacity;
} HelpArticleVector;

typedef struct HelpKeywordEntry {
  char *keyword; /* lowercased copy, owned by this entry */
  size_t article_index;
} HelpKeywordEntry;
