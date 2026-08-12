#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * netcommon.c
 */

/*
 * This file contains routines used by the networking code that do not
 * depend on the implementation of the networking code.  The network-specific
 * portions of the descriptor data structure are not used.
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "btech/context.h"
#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/network/connection_commands.h"
#include "mux/network/descriptor.h"
#include "mux/network/network_output.h"
#include "mux/network/telnet_environment.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/render.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

void make_portlist(DescriptorRegistry *descriptors, DbRef target, char *buff,
                   char **bufc) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  int i = 0;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (d->player == target) {
      safe_str(tprintf("%d ", d->descriptor), buff, bufc);
      i = 1;
    }
  }
  if (i) {
    size_t length = strlen(buff);

    *bufc = checked_mutable_string_suffix(buff, length - 1);
  }
  **bufc = '\0';
}

/*
 * ---------------------------------------------------------------------------
 * * timeval_sub: return difference between two times as a timeval
 */

static const char *time_format_1(time_t dt) {
  register struct tm *delta;
  static char buf[64];

  if (dt < 0)
    dt = 0;

  delta = gmtime(&dt);
  if (delta->tm_yday > 0) {
    (void)snprintf(buf, sizeof(buf), "%dd %02d:%02d", delta->tm_yday,
                   delta->tm_hour, delta->tm_min);
  } else {
    (void)snprintf(buf, sizeof(buf), "%02d:%02d", delta->tm_hour,
                   delta->tm_min);
  }
  return buf;
}

static const char *time_format_2(time_t dt) {
  register struct tm *delta;
  static char buf[64];

  if (dt < 0)
    dt = 0;

  delta = gmtime(&dt);
  if (delta->tm_yday > 0) {
    (void)snprintf(buf, sizeof(buf), "%dd", delta->tm_yday);
  } else if (delta->tm_hour > 0) {
    (void)snprintf(buf, sizeof(buf), "%dh", delta->tm_hour);
  } else if (delta->tm_min > 0) {
    (void)snprintf(buf, sizeof(buf), "%dm", delta->tm_min);
  } else {
    (void)snprintf(buf, sizeof(buf), "%ds", delta->tm_sec);
  }
  return buf;
}

int fetch_idle(DescriptorRegistry *descriptors, RuntimeClock *clock,
               DbRef target) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_player(descriptors, target);
  int result, idletime;

  result = -1;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    idletime = (int)(clock->now - d->last_time);
    if ((result == -1) || (idletime < result))
      result = idletime;
  }
  return result;
}

int fetch_connect(DescriptorRegistry *descriptors, RuntimeClock *clock,
                  DbRef target) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_player(descriptors, target);
  int result, conntime;

  result = -1;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    conntime = (int)(clock->now - d->connected_at);
    if (conntime > result)
      result = conntime;
  }
  return result;
}

static char *trimmed_name(GameDatabase *database, DbRef player) {
  static char cbuff[18];
  char *name = game_object_pure_name(database, player);

  if (strlen(name) <= 16)
    return name;
  string_copy_trunc(cbuff, name, 16);
  cbuff[16] = '\0';
  return cbuff;
}

static void dump_users(Descriptor *e, const char *match) {
  CommandRuntime *runtime = descriptor_runtime(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(runtime->descriptors);
  int count;
  char *buf, flist[4], slist[4];
  size_t flist_length;
  size_t slist_length;

  if (match) {
    size_t match_length = strlen(match);
    size_t offset = 0;

    while (offset < match_length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               match, match_length, sizeof(char), offset)))
      offset++;
    match = checked_string_suffix(match, offset);
  }
  if (!match || !*match)
    match = nullptr;

  buf = alloc_lbuf("dump_users");
  descriptor_queue_string(e, "Player Name         On For  Idle ");
  descriptor_queue_string(e, "     Room    Cmds Host\r\n");
  count = 0;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (match &&
        !(string_prefix(
            game_object_pure_name(runtime->world->database, d->player), match)))
      continue;
    count++;

    flist_length = 0;
    slist_length = 0;
    if (is_hidden(runtime->world->database, d->player)) {
      if (d->is_autodark)
        *(char *)checked_storage_at(flist, sizeof(flist), sizeof(char),
                                    flist_length++) = 'd';
      else if (is_dark(runtime->world->database, d->player))
        *(char *)checked_storage_at(flist, sizeof(flist), sizeof(char),
                                    flist_length++) = 'D';
    }
    if (is_suspect(runtime->world->database, d->player))
      *(char *)checked_storage_at(flist, sizeof(flist), sizeof(char),
                                  flist_length++) = '+';
    if (d->host_info & H_FORBIDDEN)
      *(char *)checked_storage_at(slist, sizeof(slist), sizeof(char),
                                  slist_length++) = 'F';
    if (d->host_info & H_SUSPECT)
      *(char *)checked_storage_at(slist, sizeof(slist), sizeof(char),
                                  slist_length++) = '+';
    *(char *)checked_storage_at(flist, sizeof(flist), sizeof(char),
                                flist_length) = '\0';
    *(char *)checked_storage_at(slist, sizeof(slist), sizeof(char),
                                slist_length) = '\0';

    (void)snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%-3s#%6ld %7d %-25s\r\n",
                   trimmed_name(runtime->world->database, d->player),
                   time_format_1(runtime->clock->now - d->connected_at),
                   time_format_2(runtime->clock->now - d->last_time), flist,
                   game_object_location(runtime->world->database, d->player),
                   d->command_count,
                   (d->username[0] != '\0')
                       ? tprintf("%s@%s", d->username, d->addr)
                       : d->addr);
    descriptor_queue_string(e, buf);
  }
  (void)snprintf(
      buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n", count,
      (count == 1) ? " " : "s ", *descriptor_runtime(e)->record_players,
      (runtime->world->configuration->max_players == -1)
          ? "no"
          : tprintf("%d", runtime->world->configuration->max_players));

  descriptor_queue_string(e, buf);

  free_lbuf(buf);
}

static void dump_sessions(Descriptor *e, const char *match) {
  CommandRuntime *runtime = descriptor_runtime(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(runtime->descriptors);
  int count;
  char *buf;

  if (match) {
    size_t match_length = strlen(match);
    size_t offset = 0;

    while (offset < match_length &&
           (isspace)(*(const unsigned char *)checked_storage_at_const(
               match, match_length, sizeof(char), offset)))
      offset++;
    match = checked_string_suffix(match, offset);
  }
  if (!match || !*match)
    match = nullptr;

  buf = alloc_lbuf("dump_sessions");
  descriptor_queue_string(e, "                               ");
  descriptor_queue_string(
      e, "     Characters Input----  Characters Output---\r\n");
  descriptor_queue_string(e, "Player Name         On For  Idle ");
  descriptor_queue_string(
      e, "Port Pend  Lost     Total  Pend  Lost     Total\r\n");

  count = 0;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (match &&
        !string_prefix(
            game_object_pure_name(runtime->world->database, d->player), match))
      continue;
    count++;

    (void)snprintf(
        buf, LBUF_SIZE, "%-16s%10s %5s%5d%5d%6d%10d%6d%6d%10d\r\n",
        trimmed_name(runtime->world->database, d->player),
        time_format_1(runtime->clock->now - d->connected_at),
        time_format_2((runtime->clock->now - d->last_time) > HIDDEN_IDLESECS
                          ? (runtime->clock->now - d->last_time)
                          : 0),
        d->descriptor, d->input_size, d->input_lost, d->input_tot,
        d->output_size, d->output_lost, d->output_tot);
    descriptor_queue_string(e, buf);
  }

  (void)snprintf(
      buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n", count,
      (count == 1) ? " " : "s ", *descriptor_runtime(e)->record_players,
      (runtime->world->configuration->max_players == -1)
          ? "no"
          : tprintf("%d", runtime->world->configuration->max_players));
  descriptor_queue_string(e, buf);
  free_lbuf(buf);
}

void do_who(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  Descriptor *descriptor = invocation->context->descriptor;
  char *match = invocation->first;

  if (descriptor == nullptr) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "@who is only available from an active connection.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  dump_users(descriptor, match);
}

static const char *terminal_color_depth_name(TerminalColorDepth depth) {
  switch (depth) {
  case TERMINAL_COLOR_NONE:
    return "off";
  case TERMINAL_COLOR_ANSI_16:
    return "16";
  case TERMINAL_COLOR_ANSI_256:
    return "256";
  case TERMINAL_COLOR_TRUECOLOR:
    return "truecolor";
  }
  return "unknown";
}

static int
telnet_environment_view_compare(const ArraySortComparison *comparison) {
  const TelnetEnvironmentEntryView *left_entry = comparison->left;
  const TelnetEnvironmentEntryView *right_entry = comparison->right;
  size_t shared_size;
  int ordering;

  if (left_entry->kind != right_entry->kind)
    return left_entry->kind < right_entry->kind ? -1 : 1;
  shared_size = left_entry->name_size < right_entry->name_size
                    ? left_entry->name_size
                    : right_entry->name_size;
  ordering = memcmp(left_entry->name, right_entry->name, shared_size);
  if (ordering != 0)
    return ordering;
  if (left_entry->name_size == right_entry->name_size)
    return 0;
  return left_entry->name_size < right_entry->name_size ? -1 : 1;
}

static void telnet_append_escaped(char *buffer, char **position,
                                  const unsigned char *value, size_t size) {
  for (size_t index = 0; index < size; index++) {
    unsigned char byte = *(const unsigned char *)checked_storage_at_const(
        value, size, sizeof(*value), index);

    if (byte == '\\')
      safe_str("\\\\", buffer, position);
    else if (byte == '"')
      safe_str("\\\"", buffer, position);
    else if (byte >= 0x20 && byte <= 0x7e)
      safe_chr((char)byte, buffer, position);
    else
      safe_str(tprintf("\\x%02X", byte), buffer, position);
  }
}

static TelnetEnvironmentEntryView *
telnet_environment_view_at(TelnetEnvironmentEntryView *entries, size_t count,
                           size_t index) {
  return checked_storage_at(entries, count, sizeof(*entries), index);
}

static void dump_telnet_environment(EvaluationContext *evaluation, DbRef viewer,
                                    const Descriptor *descriptor) {
  constexpr size_t DISPLAY_CHUNK_SIZE = 3500;
  TelnetEnvironmentEntryView entries[64];
  size_t count = descriptor_telnet_environment_count(descriptor);
  char *buffer = alloc_lbuf("dump_telnet_environment");

  notify_checked(evaluation, viewer, viewer,
                 "  NEW-ENVIRON:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_new_environ_enabled ? "yes" : "no");
  if (count == 0) {
    notify_checked(evaluation, viewer, viewer, "    Variables: (none)",
                   MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buffer);
    return;
  }
  if (count > sizeof(entries) / sizeof(entries[0]))
    count = sizeof(entries) / sizeof(entries[0]);
  for (size_t index = 0; index < count; index++)
    descriptor_telnet_environment_entry(
        descriptor, index, telnet_environment_view_at(entries, count, index));
  array_sort(&(ArraySortRequest){.items = entries,
                                 .count = count,
                                 .item_size = sizeof(entries[0]),
                                 .compare = telnet_environment_view_compare});
  notify_checked(evaluation, viewer, viewer,
                 "    Variables:", MSG_ME_ALL | MSG_F_DOWN);
  for (size_t index = 0; index < count; index++) {
    TelnetEnvironmentEntryView *entry =
        telnet_environment_view_at(entries, count, index);
    size_t value_position = 0;
    bool first_chunk = true;

    do {
      size_t remaining = entry->value_size - value_position;
      size_t chunk_size =
          remaining < DISPLAY_CHUNK_SIZE ? remaining : DISPLAY_CHUNK_SIZE;
      char *position = buffer;

      if (first_chunk) {
        safe_str(entry->kind == TELNET_ENVIRONMENT_VAR ? "      VAR \""
                                                       : "      USERVAR \"",
                 buffer, &position);
        telnet_append_escaped(buffer, &position, entry->name, entry->name_size);
        safe_str("\" = \"", buffer, &position);
      } else {
        safe_str("        value += \"", buffer, &position);
      }
      telnet_append_escaped(
          buffer, &position,
          checked_storage_region_const(entry->value, entry->value_size,
                                       value_position, chunk_size),
          chunk_size);
      safe_chr('"', buffer, &position);
      *position = '\0';
      notify_checked(evaluation, viewer, viewer, buffer,
                     MSG_ME_ALL | MSG_F_DOWN);
      value_position += chunk_size;
      first_chunk = false;
    } while (value_position < entry->value_size);
  }
  free_lbuf(buffer);
}

static void dump_telnet_descriptor(EvaluationContext *evaluation, DbRef viewer,
                                   const Descriptor *descriptor) {
  char *client = alloc_lbuf("dump_telnet_client");
  char *client_position = client;
  char *terminal = alloc_lbuf("dump_telnet_terminal");
  char *terminal_position = terminal;

  telnet_append_escaped(client, &client_position,
                        (const unsigned char *)descriptor->terminal_client,
                        strlen(descriptor->terminal_client));
  *client_position = '\0';
  telnet_append_escaped(terminal, &terminal_position,
                        (const unsigned char *)descriptor->terminal_type,
                        strlen(descriptor->terminal_type));
  *terminal_position = '\0';
  notify_printf(
      evaluation, viewer, "Telnet state for %s(#%ld), descriptor %d:",
      game_object_name(evaluation->world->database, descriptor->player),
      descriptor->player, descriptor->descriptor);
  notify_checked(evaluation, viewer, viewer,
                 "  TTYPE / MTTS:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_ttype_enabled ? "yes" : "no");
  notify_printf(evaluation, viewer, "    Client: \"%s\"", client);
  notify_printf(evaluation, viewer, "    Terminal type: \"%s\"", terminal);
  notify_printf(evaluation, viewer, "    Responses: %d",
                descriptor->terminal_type_responses);
  notify_printf(evaluation, viewer, "    Color depth: %s",
                terminal_color_depth_name(descriptor->terminal_color_depth));
  notify_printf(evaluation, viewer, "    Screen reader: %s",
                descriptor->is_screen_reader ? "yes" : "no");
  notify_checked(evaluation, viewer, viewer,
                 "  NAWS:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_naws_enabled ? "yes" : "no");
  notify_printf(evaluation, viewer, "    Window size: %dx%d",
                descriptor->terminal_width, descriptor->terminal_height);
  notify_checked(evaluation, viewer, viewer,
                 "  CHARSET:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_charset_enabled ? "yes" : "no");
  notify_printf(evaluation, viewer, "    Encoding: %s",
                descriptor->is_charset_utf8 ? "UTF-8" : "unsupported");
  notify_printf(evaluation, viewer, "    Request pending: %s",
                descriptor->is_charset_request_pending ? "yes" : "no");
  dump_telnet_environment(evaluation, viewer, descriptor);
  notify_checked(evaluation, viewer, viewer,
                 "  GMCP:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_gmcp_enabled ? "yes" : "no");
  notify_checked(evaluation, viewer, viewer,
                 "  MSSP:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_mssp_enabled ? "yes" : "no");
  notify_checked(evaluation, viewer, viewer,
                 "  MCCP2:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Compression: %s",
                descriptor->is_mccp_enabled ? "active" : "inactive");
  notify_checked(evaluation, viewer, viewer,
                 "  ECHO:", MSG_ME_ALL | MSG_F_DOWN);
  notify_printf(evaluation, viewer, "    Client echo: %s",
                descriptor->is_echo_suppressed ? "suppressed" : "enabled");
  free_lbuf(terminal);
  free_lbuf(client);
}

void do_telnet(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef target;
  Descriptor *descriptor;
  DescriptorIterator iterator;
  int count = 0;

  if (invocation->first == nullptr || invocation->first[0] == '\0') {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Usage: @telnet <player>", MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  target = lookup_player(invocation->context->world, invocation->player,
                         invocation->first, 0);
  if (target == NOTHING) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "No such player.", MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  iterator =
      descriptor_iterator_player(evaluation->runtime->descriptors, target);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    dump_telnet_descriptor(evaluation, invocation->player, descriptor);
    count++;
  }
  if (count == 0)
    notify_checked(evaluation, invocation->player, invocation->player,
                   "That player is not connected.", MSG_ME_ALL | MSG_F_DOWN);
}

void do_color(CommandInvocation *invocation) {
  Descriptor *descriptor = invocation->context->descriptor;
  const char *mode = invocation->first;
  TerminalColorDepth requested;

  if (descriptor == nullptr || descriptor->player != invocation->player) {
    notify_checked(&invocation->context->evaluation, invocation->player,
                   invocation->player,
                   "color is only available from an active connection.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if (!mode || !*mode) {
    notify_printf(&invocation->context->evaluation, invocation->player,
                  "Color mode: %s%s. Client capability: %s%s.",
                  descriptor->has_color_override
                      ? terminal_color_depth_name(descriptor->color_override)
                      : "auto",
                  descriptor->has_color_override ? " (override)" : "",
                  terminal_color_depth_name(descriptor->terminal_color_depth),
                  descriptor->is_screen_reader ? ", screen reader" : "");
    return;
  }
  if (!strcasecmp(mode, "auto")) {
    descriptor->has_color_override = false;
  } else {
    if (!strcasecmp(mode, "off")) {
      requested = TERMINAL_COLOR_NONE;
    } else if (!strcmp(mode, "16")) {
      requested = TERMINAL_COLOR_ANSI_16;
    } else if (!strcmp(mode, "256")) {
      requested = TERMINAL_COLOR_ANSI_256;
    } else if (!strcasecmp(mode, "truecolor")) {
      requested = TERMINAL_COLOR_TRUECOLOR;
    } else {
      notify_checked(&invocation->context->evaluation, invocation->player,
                     invocation->player,
                     "Use color auto, off, 16, 256, or truecolor.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    descriptor->has_color_override = true;
    descriptor->color_override = requested;
  }
  notify_printf(&invocation->context->evaluation, invocation->player,
                "Color mode set to %s.",
                descriptor->has_color_override
                    ? terminal_color_depth_name(descriptor->color_override)
                    : "auto");
}

void do_session(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  Descriptor *descriptor = invocation->context->descriptor;
  char *match = invocation->first;

  if (descriptor == nullptr) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "@session is only available from an active connection.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  dump_sessions(descriptor, match);
}

void do_quit(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  Descriptor *descriptor = invocation->context->descriptor;

  if (descriptor == nullptr || descriptor->player != player) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "quit is only available from an active connection.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_QUIT);
}

int descriptor_command(Descriptor *d, char *command) {
  CommandRuntime *runtime = descriptor_runtime(d);
  CommandContext context;

  if (!command_context_initialize(
          &(CommandContextInitialization){.context = &context,
                                          .runtime = runtime,
                                          .btech = descriptor_btech(d),
                                          .log = descriptor_log(d),
                                          .player = d->player,
                                          .enactor = d->player,
                                          .descriptor = d,
                                          .interactive = true}))
    return 0;
  context.debug_command = "< descriptor_command >";

  /* The IDLE command is used to keep players behind badly configured NATs
     alive. This does not increment command count or idle time and is a
     good alternative to a lot of the current anti-disconnectors out there.
  */
  if (!strcasecmp(command, "IDLE") && d->is_connected) {
    context.debug_command = "idle";
    command_context_destroy(&context);
    return 1;
  }

  d->last_time = runtime->clock->now;
  d->command_count++;
  BtechCommandScope btech_scope;
  btech_command_scope_enter(&btech_scope, context.btech, &context);
  process_command(&context, command, (char **)nullptr, 0);
  btech_command_scope_leave(&btech_scope);
  command_context_destroy(&context);
  return 1;
}

/*
 * --------------------------------------------------------------------------
 * * site_data_check: Check for site flags in a site list.
 */
