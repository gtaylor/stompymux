/* help_command.c - `help` and `@helpreload` command handlers. */

#include "mux/server/platform.h"

#include <string.h>

#include "mux/database/flags.h"
#include "mux/help/help_command.h"
#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"

static void help_command_send_article(DbRef player, const HelpArticle *article,
                                      bool viewer_is_wizard) {
  HelpTextBuffer buffer;

  help_text_buffer_init(&buffer);
  help_article_render_body(article, viewer_is_wizard, &buffer);
  help_render_send(player, &buffer);
  help_text_buffer_free(&buffer);
}

static void help_command_send_suggestions(DbRef player, const char *needle,
                                          bool viewer_is_wizard) {
  size_t total = help_index_keyword_count();
  char *topic_list;
  char *bp;
  bool any = false;
  size_t i;

  topic_list = alloc_lbuf("do_help.suggestions");
  bp = topic_list;
  for (i = 0; i < total; i++) {
    const HelpArticle *owner = help_index_keyword_article_at(i);

    if (owner->wizard_only && !viewer_is_wizard)
      continue;
    if (strstr(help_index_keyword_at(i), needle)) {
      safe_str(help_index_keyword_at(i), topic_list, &bp);
      safe_str("  ", topic_list, &bp);
      any = true;
    }
  }
  *bp = '\0';
  if (!any)
    notify_printf(player, "No help found for '%s'.", needle);
  else {
    notify_printf(player, "No exact match for '%s'. Did you mean:", needle);
    notify(player, topic_list);
  }
  free_lbuf(topic_list);
}

void do_help(DbRef player, DbRef cause, int key, char *message) {
  bool viewer_is_wizard = is_wizard(player);
  const HelpArticle *article;
  char *p;

  (void)cause;
  (void)key;

  if (*message == '\0') {
    article = help_index_default_article();
    if (!article) {
      notify(player, "Unable to render default help article");
      return;
    }
    help_command_send_article(player, article, viewer_is_wizard);
    return;
  }

  for (p = message; *p; p++)
    *p = ToLower(*p);

  article = help_index_find_exact(message, viewer_is_wizard);
  if (article) {
    help_command_send_article(player, article, viewer_is_wizard);
    return;
  }

  help_command_send_suggestions(player, message, viewer_is_wizard);
}

void do_helpreload(DbRef player, DbRef cause, int key) {
  (void)cause;
  (void)key;

  help_index_reload(player);
}
