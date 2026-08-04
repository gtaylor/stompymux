/*
 * netcommon.c
 */

/*
 * This file contains routines used by the networking code that do not
 * depend on the implementation of the networking code.  The network-specific
 * portions of the descriptor data structure are not used.
 */

#include "mux/server/platform.h"

#include <arpa/inet.h>
#include <time.h>

#include "btech/btech_context.h"
#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_runtime.h"
#include "mux/communication/comsys.h"
#include "mux/network/network_output.h"
#include "mux/network/telnet_environment.h"
#include "mux/network/telnet_socket.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/render.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

void raw_notify_raw(EvaluationContext *evaluation, DbRef player,
                    const char *msg, const char *append) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_player(evaluation->runtime->descriptors, player);

  if (!msg || !*msg)
    return;

  if (!is_connected(evaluation->world->database, player))
    return;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    descriptor_queue_string(d, msg);
    if (append != nullptr)
      descriptor_queue_write(d, append, (int)strlen(append));
  }
}

/* raw_notify: write a message to a player */
void raw_notify(EvaluationContext *evaluation, DbRef player, const char *msg) {
  raw_notify_raw(evaluation, player, msg, "\r\n");
}

void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_player(evaluation->runtime->descriptors, player);
  char buffer[LBUF_SIZE];
  va_list ap;
  memset(buffer, 0, LBUF_SIZE);

  va_start(ap, format);

  vsnprintf(buffer, LBUF_SIZE - 1, format, ap);
  va_end(ap);

  strncat(buffer, "\r\n", LBUF_SIZE - 1);
  buffer[LBUF_SIZE - 1] = '\0';

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    descriptor_queue_string(d, buffer);
  }
}

void raw_notify_newline(EvaluationContext *evaluation, DbRef player) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_player(evaluation->runtime->descriptors, player);

  if (!is_connected(evaluation->world->database, player))
    return;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    descriptor_queue_write(d, "\r\n", 2);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * raw_broadcast: Send message to players who have indicated flags
 */

void raw_broadcast(DescriptorRegistry *descriptors, int inflags,
                   const char *template, ...) {
  char buff[LBUF_SIZE];
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  va_list ap;

  if (!template || !*template)
    return;

  va_start(ap, template);
  vsnprintf(buff, LBUF_SIZE, template, ap);
  buff[LBUF_SIZE - 1] = '\0';

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (inflags == OBJECT_FLAG_NONE ||
        game_object_has_flag(descriptor_runtime(d)->world->database, d->player,
                             (ObjectFlag)inflags)) {
      descriptor_queue_string(d, buff);
      descriptor_queue_write(d, "\r\n", 2);
    }
  }
  va_end(ap);
}

/*
 * ---------------------------------------------------------------------------
 * * descriptor_queue_write: Add text to the output queue for the indicated
 * descriptor.
 */

void descriptor_queue_write(Descriptor *d, const char *b, int n) {
  if (n <= 0)
    return;

  descriptor_write(d, b, (size_t)n);
  d->output_tot += n;
  return;
}

void descriptor_queue_string(Descriptor *d, const char *s) {
  char rendered[LBUF_SIZE];
  StyledTextRenderOptions options = {0};

  if (is_ansi(descriptor_runtime(d)->world->database, d->player)) {
    if (d->has_color_override)
      options.color_depth = d->color_override;
    else if (!d->is_screen_reader)
      options.color_depth = d->terminal_color_depth;
  }
  options.osc_hyperlinks = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS");
  options.osc_hyperlinks_send = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_SEND");
  options.osc_hyperlinks_prompt = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_PROMPT");
  options.osc_hyperlinks_style_basic =
      descriptor_telnet_environment_value_is_one(d, TELNET_ENVIRONMENT_USERVAR,
                                                 "OSC_HYPERLINKS_STYLE_BASIC");
  options.osc_hyperlinks_style_states =
      descriptor_telnet_environment_value_is_one(d, TELNET_ENVIRONMENT_USERVAR,
                                                 "OSC_HYPERLINKS_STYLE_STATES");
  options.osc_hyperlinks_tooltip = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_TOOLTIP");
  options.osc_hyperlinks_menu = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_MENU");
  options.osc_hyperlinks_visibility =
      descriptor_telnet_environment_value_is_one(d, TELNET_ENVIRONMENT_USERVAR,
                                                 "OSC_HYPERLINKS_VISIBILITY");
  options.osc_hyperlinks_spoiler = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_SPOILER");
  options.osc_hyperlinks_disabled = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_DISABLED");
  options.osc_hyperlinks_selection = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_SELECTION");
  options.osc_hyperlinks_compact = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_COMPACT");
  options.osc_hyperlinks_presets = descriptor_telnet_environment_value_is_one(
      d, TELNET_ENVIRONMENT_USERVAR, "OSC_HYPERLINKS_PRESETS");
  if (options.osc_hyperlinks_presets && !d->osc8_presets_emitted) {
    d->osc8_presets_emitted = true;
    size_t count = styled_text_palette_preset_count(
        descriptor_runtime(d)->world->styled_text_palette);
    for (size_t index = 0; index < count; index++) {
      char definition[LBUF_SIZE];

      if (styled_text_palette_render_preset(
              descriptor_runtime(d)->world->styled_text_palette, index,
              &options, definition, sizeof(definition)))
        descriptor_queue_write(d, definition, (int)strlen(definition));
    }
  }
  styled_text_render_with_options(
      descriptor_runtime(d)->world->styled_text_palette, s, &options, rendered,
      sizeof(rendered));
  descriptor_queue_write(d, rendered, (int)strlen(rendered));
}
