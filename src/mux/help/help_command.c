/* help_command.c - `help` and `@help` command handlers. */

#include <string.h>

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/help/help_command.h"
#include "mux/help/help_index.h"
#include "mux/help/help_render.h"
#include "mux/help/help_types.h"
#include "mux/network/network_output.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"

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
  if (!any) {
    notify_printf(evaluation, player, "No help found for '%s'.", needle);
  } else {
    notify_printf(evaluation, player,
                  "No exact match for '%s'. Did you mean:", needle);
    notify_checked(evaluation, player, player, topic_list,
                   MSG_ME_ALL | MSG_F_DOWN);
  }
  free_lbuf(topic_list);
}

void do_help(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *message = invocation->first;
  HelpIndex *help = invocation->context->runtime->help;
  bool viewer_is_wizard =
      is_wizard(invocation->context->world->database, player);
  const HelpArticle *article;

  if (*message == '\0') {
    article = help_index_default_article(help);
    if (!article) {
      notify_checked(&invocation->context->evaluation, player, player,
                     "Unable to render default help article",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    help_command_send_article(&invocation->context->evaluation, help, player,
                              article, viewer_is_wizard);
    return;
  }

  const size_t LENGTH = strlen(message);

  for (size_t index = 0; index < LENGTH; index++) {
    char *character =
        checked_storage_at(message, LENGTH + 1, sizeof(char), index);

    *character = ascii_to_lower(*character);
  }

  article = help_index_find_exact(help, message, viewer_is_wizard);
  if (article) {
    help_command_send_article(&invocation->context->evaluation, help, player,
                              article, viewer_is_wizard);
    return;
  }

  help_command_send_suggestions(&invocation->context->evaluation, help, player,
                                message, viewer_is_wizard);
}

static void do_help_reload(CommandInvocation *invocation) {
  help_index_reload(&invocation->context->evaluation,
                    invocation->context->runtime->help, invocation->player);
}

void do_help_admin(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  switch (invocation->key) {
  case 0:
    raw_notify(evaluation, invocation->player, "@help command switches:");
    raw_notify(evaluation, invocation->player,
               "  /reload  Rebuild the help index.");
    return;
  case HELP_COMMAND_RELOAD:
    do_help_reload(invocation);
    return;
  default:
    raw_notify(evaluation, invocation->player,
               "Invalid @help switch combination.");
    return;
  }
}
