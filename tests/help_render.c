/* help_render.c -- Plain-text markdown rendering unit test */

#include <string.h>

#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/game.h"

/*
 * help_render_markdown() never calls these (the render test never invokes
 * help_article_render_body or help_render_send), but the object file
 * references them; stub them out so this test can link help_render.c
 * without pulling in help_index.c and its server-wide dependencies.
 */
char *help_index_read_body(const HelpIndex *index, const HelpArticle *article,
                           size_t *out_length) {
  (void)index;
  (void)article;
  (void)out_length;
  return nullptr;
}

size_t help_index_article_count(const HelpIndex *index) {
  (void)index;
  return 0;
}

const HelpArticle *help_index_article_at(const HelpIndex *index,
                                         size_t article_index) {
  (void)index;
  (void)article_index;
  return nullptr;
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

int main(void) {
  if (!help_render_test_expect("# Header 1\n\nContent here\n\n## Header 2\n",
                               "# Header 1\n\nContent here\n\n## Header 2"))
    return 1;
  if (!help_render_test_expect("[text](http://example.com)", "text"))
    return 2;
  if (!help_render_test_expect("*em* and **strong**", "em and strong"))
    return 3;
  if (!help_render_test_expect("- one\n- two\n", "- one\n- two"))
    return 4;
  return 0;
}
