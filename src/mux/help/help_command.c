/* help_command.c - `help` and `@helpreload` command handlers. */

#include "mux/server/platform.h"

#include <string.h>

#include "mux/database/flags.h"
#include "mux/help/help_command.h"
#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

static void help_command_send_article(EvaluationContext *evaluation,
                                      HelpIndex *help, DbRef player,
                                      const HelpArticle *article,
                                      bool viewer_is_wizard) {
  HelpTextBuffer buffer;

  help_text_buffer_init(&buffer);
  help_article_render_body(help, article, viewer_is_wizard, &buffer);
  help_render_send(evaluation, player, &buffer);
  help_text_buffer_free(&buffer);
}

static void help_command_send_suggestions(EvaluationContext *evaluation,
                                          HelpIndex *help, DbRef player,
                                          const char *needle,
                                          bool viewer_is_wizard) {
  size_t total = help_index_keyword_count(help);
  char *topic_list;
  char *bp;
  bool any = false;
  size_t i;

  topic_list = alloc_lbuf("do_help.suggestions");
  bp = topic_list;
  for (i = 0; i < total; i++) {
    const HelpArticle *owner = help_index_keyword_article_at(help, i);

    if (owner->wizard_only && !viewer_is_wizard)
      continue;
    if (strstr(help_index_keyword_at(help, i), needle)) {
      safe_str(help_index_keyword_at(help, i), topic_list, &bp);
      safe_str("  ", topic_list, &bp);
      any = true;
    }
  }
  *bp = '\0';
  if (!any)
    notify_printf(evaluation, player, "No help found for '%s'.", needle);
  else {
    notify_printf(evaluation, player,
                  "No exact match for '%s'. Did you mean:", needle);
    notify(evaluation, player, topic_list);
  }
  free_lbuf(topic_list);
}

void do_help(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *message = invocation->first;
  HelpIndex *help = invocation->context->server->help;
  bool viewer_is_wizard =
      is_wizard(invocation->context->world->database, player);
  const HelpArticle *article;
  char *p;

  if (*message == '\0') {
    article = help_index_default_article(help);
    if (!article) {
      notify(&invocation->context->evaluation, player,
             "Unable to render default help article");
      return;
    }
    help_command_send_article(&invocation->context->evaluation, help, player,
                              article, viewer_is_wizard);
    return;
  }

  for (p = message; *p; p++)
    *p = ToLower(*p);

  article = help_index_find_exact(help, message, viewer_is_wizard);
  if (article) {
    help_command_send_article(&invocation->context->evaluation, help, player,
                              article, viewer_is_wizard);
    return;
  }

  help_command_send_suggestions(&invocation->context->evaluation, help, player,
                                message, viewer_is_wizard);
}

void do_helpreload(CommandInvocation *invocation) {
  help_index_reload(&invocation->context->evaluation,
                    invocation->context->server->help, invocation->player);
}
