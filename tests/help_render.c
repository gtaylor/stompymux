/* help_render.c -- Plain-text markdown rendering unit test */

#include <stdlib.h>
#include <string.h>

#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/game.h"
#include "mux/support/checked_storage.h"

static const HelpArticle *test_articles;
static size_t test_article_count;

char *help_index_read_body(const HelpIndex *index, const HelpArticle *article,
                           size_t *out_length) {
  (void)index;
  (void)article;
  char *body = malloc(1);

  body[0] = '\0';
  if (out_length)
    *out_length = 0;
  return body;
}

size_t help_index_article_count(const HelpIndex *index) {
  (void)index;
  return test_article_count;
}

const HelpArticle *help_index_article_at(const HelpIndex *index,
                                         size_t article_index) {
  (void)index;
  return checked_storage_at_const(test_articles, test_article_count,
                                  sizeof(*test_articles), article_index);
}

void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *message, int key) {
  (void)evaluation;
  (void)target;
  (void)sender;
  (void)message;
  (void)key;
}

static int help_render_test_expect(const char *markdown, const char *expected) {
  HelpTextBuffer buffer;
  int ok;

  help_text_buffer_init(&buffer);
  help_render_markdown(markdown, strlen(markdown), &buffer);
  ok = buffer.data != nullptr && !strcmp(buffer.data, expected);
  help_text_buffer_free(&buffer);
  return ok;
}

static int help_render_test_index_links(void) {
  char index_tag[] = "index-entry";
  char index_keyword[] = "index";
  char topic_tag[] = "index-entry";
  char topic_keyword[] = "topic name";
  char *index_tags[] = {index_tag};
  char *index_keywords[] = {index_keyword};
  char *topic_tags[] = {topic_tag};
  char *topic_keywords[] = {topic_keyword};
  char description[] = "A linked help topic.";
  HelpArticle articles[] = {
      {
          .keywords = {.items = index_keywords, .count = 1},
          .show_index_for_article_tags = {.items = index_tags, .count = 1},
      },
      {
          .description = description,
          .keywords = {.items = topic_keywords, .count = 1},
          .article_tags = {.items = topic_tags, .count = 1},
      },
  };
  HelpTextBuffer buffer;
  int ok;

  test_articles = articles;
  test_article_count = sizeof(articles) / sizeof(articles[0]);
  help_text_buffer_init(&buffer);
  help_article_render_body(nullptr, &articles[0], false, &buffer);
  ok = buffer.data != nullptr &&
       !strcmp(buffer.data, "TOPIC                DESCRIPTION\n"
                            "[send=\"help topic name\"]topic name[/]           "
                            "A linked help topic.\n");
  help_text_buffer_free(&buffer);
  articles[0].index_style = HELP_INDEX_STYLE_COLUMNAR;
  help_text_buffer_init(&buffer);
  help_article_render_body(nullptr, &articles[0], false, &buffer);
  ok = ok && buffer.data != nullptr &&
       !strcmp(buffer.data,
               "[send=\"help topic name\"]topic name[/]          \n");
  help_text_buffer_free(&buffer);
  test_articles = nullptr;
  test_article_count = 0;
  return ok;
}

int main(void) {
  if (!help_render_test_expect("# Header 1\n\nContent here\n\n## Header 2\n",
                               "# Header 1\n\nContent here\n\n## Header 2"))
    return 1;
  if (!help_render_test_expect("[text](http://example.com)",
                               "[link=\"http://example.com\"]text[/]"))
    return 2;
  if (!help_render_test_expect("[local](../topic/) and [bad](file:///tmp/a)",
                               "local and bad"))
    return 7;
  if (!help_render_test_expect("*em* and **strong**", "em and strong"))
    return 3;
  if (!help_render_test_expect("- one\n- two\n", "- one\n- two"))
    return 4;
  if (!help_render_test_expect(
          "Rendered [fg=red]red[/], but `[fg=blue]blue[/]` is code.",
          "Rendered [fg=red]red[/], but [[fg=blue]blue[[/] is code."))
    return 5;
  if (!help_render_test_expect("```text\n"
                               "@name drone=[fg=bright-cyan]Aegis[/]\n"
                               "```\n",
                               "@name drone=[[fg=bright-cyan]Aegis[[/]\n"))
    return 6;
  if (!help_render_test_index_links())
    return 8;
  return 0;
}
