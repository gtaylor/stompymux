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
#include "mux/network/netcommon.h"
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

/*
 * ---------------------------------------------------------------------------
 * * make_portlist: Make a list of ports for PORTS().
 */

void make_portlist(DescriptorRegistry *descriptors, DbRef player, DbRef target,
                   char *buff, char **bufc) {
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
    (*bufc)--;
  }
  **bufc = '\0';
}

/*
 * ---------------------------------------------------------------------------
 * * timeval_sub: return difference between two times as a timeval
 */

struct timeval timeval_sub(struct timeval now, struct timeval then) {
  now.tv_sec -= then.tv_sec;
  now.tv_usec -= then.tv_usec;
  if (now.tv_usec < 0) {
    now.tv_usec += 1000000;
    now.tv_sec--;
  }
  return now;
}

/*
 * ---------------------------------------------------------------------------
 * * msec_diff: return difference between two times in msec
 */

int msec_diff(struct timeval now, struct timeval then) {
  return (int)((now.tv_sec - then.tv_sec) * 1000 +
               (now.tv_usec - then.tv_usec) / 1000);
}

/*
 * ---------------------------------------------------------------------------
 * * msec_add: add milliseconds to a timeval
 */

struct timeval msec_add(struct timeval t, int x) {
  t.tv_sec += x / 1000;
  t.tv_usec += (x % 1000) * 1000;
  if (t.tv_usec >= 1000000) {
    t.tv_sec += t.tv_usec / 1000000;
    t.tv_usec = t.tv_usec % 1000000;
  }
  return t;
}

/*
 * ---------------------------------------------------------------------------
 * * update_quotas: Refill command quotas
 */

struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current) {
  int nslices;
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_all(descriptors);

  nslices =
      msec_diff(current, last) / (configuration->command_quota_interval > 0
                                      ? configuration->command_quota_interval
                                      : 1);

  if (nslices > 0) {
    while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
      if (d->is_dead)
        continue;
      d->quota += configuration->command_quota_increment * nslices;
      if (d->quota > configuration->command_quota_max)
        d->quota = configuration->command_quota_max;
    }
  }
  return msec_add(last, nslices * configuration->command_quota_interval);
}

/* raw_notify_raw: write a message to a player without the newline */

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

void descriptor_welcome(Descriptor *d) {
  FileCache *files = descriptor_runtime(d)->files;
  int connection_count = file_cache_connection_count(files);

  if (connection_count) {
    fcache_dump_conn(files, d, rand() % connection_count);
    return;
  }
  fcache_dump(files, d, FC_CONN);
}

void set_lastsite(Descriptor *d, char *lastsite) {
  CommandRuntime *runtime = descriptor_runtime(d);
  long flags;
  char buf[LBUF_SIZE];

  if (d->player) {
    if (lastsite) {
      strncpy(buf, lastsite, LBUF_SIZE - 1);
      buf[LBUF_SIZE - 1] = '\0';
    } else {
      attribute_get_string(runtime->world->database, buf, d->player, A_LASTSITE,
                           &flags);
    }
    attribute_add_raw(runtime->world->database, d->player, A_LASTSITE, buf);
  }
}

static const char *time_format_1(time_t dt) {
  register struct tm *delta;
  static char buf[64];

  if (dt < 0)
    dt = 0;

  delta = gmtime(&dt);
  if (delta->tm_yday > 0) {
    snprintf(buf, sizeof(buf), "%dd %02d:%02d", delta->tm_yday, delta->tm_hour,
             delta->tm_min);
  } else {
    snprintf(buf, sizeof(buf), "%02d:%02d", delta->tm_hour, delta->tm_min);
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
    snprintf(buf, sizeof(buf), "%dd", delta->tm_yday);
  } else if (delta->tm_hour > 0) {
    snprintf(buf, sizeof(buf), "%dh", delta->tm_hour);
  } else if (delta->tm_min > 0) {
    snprintf(buf, sizeof(buf), "%dm", delta->tm_min);
  } else {
    snprintf(buf, sizeof(buf), "%ds", delta->tm_sec);
  }
  return buf;
}

static void dispatch_connection_event(CommandRuntime *runtime, Descriptor *d,
                                      DbRef player, DbRef object,
                                      LuaEventType type, bool reconnect,
                                      const char *reason) {
  LuaEventInvocation invocation = {
      .type = type,
      .descriptor = d,
      .object = object,
      .enactor = player,
      .cause = player,
      .reconnect = reconnect,
      .reason = reason,
  };

  lua_event_dispatch(runtime->lua_owner->runtime, &invocation);
}

static void dispatch_connection_event_scope(CommandRuntime *runtime,
                                            Descriptor *d, DbRef player,
                                            DbRef location, LuaEventType type,
                                            bool reconnect,
                                            const char *reason) {
  DbRef object;
  DbRef zone;

  dispatch_connection_event(runtime, d, player, player, type, reconnect,
                            reason);
  if ((zone = game_object_zone(runtime->world->database, location)) == NOTHING)
    return;
  switch (typeof_obj(runtime->world->database, zone)) {
  case OBJECT_TYPE_THING:
    dispatch_connection_event(runtime, d, player, zone, type, reconnect,
                              reason);
    break;
  case OBJECT_TYPE_ROOM:
    DOLIST(runtime->world->database, object,
           game_object_contents(runtime->world->database, zone)) {
      dispatch_connection_event(runtime, d, player, object, type, reconnect,
                                reason);
    }
    break;
  default:
    log_text(tprintf("Invalid zone #%ld for %s(#%ld) has bad type %d", zone,
                     game_object_name(runtime->world->database, player), player,
                     typeof_obj(runtime->world->database, zone)));
    break;
  }
}

void announce_connect(DbRef player, Descriptor *d) {
  CommandRuntime *runtime = descriptor_runtime(d);
  const ServerConfiguration *configuration = runtime->world->configuration;
  CommandContext *command = runtime->background_command;
  DbRef loc, temp;
  long aflags;
  int num, key, count;
  char *buf, *time_str;
  Descriptor *dtemp;
  DescriptorIterator iterator;

  descriptor_queue_string(d, "Connected.\n\n");

  count = 0;
  iterator = descriptor_iterator_connected(runtime->descriptors);
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    count++;

  if (*runtime->record_players < count)
    *runtime->record_players = count;

  buf = attribute_get(runtime->world->database, player, A_TIMEOUT, &aflags);
  if (buf) {
    d->timeout = clamped_atoi(buf);
    if (d->timeout <= 0)
      d->timeout = configuration->idle_timeout;
  }
  free_lbuf(buf);

  loc = game_object_location(runtime->world->database, player);
  s_connected(runtime->world->database, player);

  if (is_wizard(runtime->world->database, player)) {
    if (!configuration->is_login_enabled) {
      raw_notify(&command->evaluation, player, "*** Logins are disabled.");
    }
  }
  buf = alloc_lbuf("announce_connect");
  num = 0;
  iterator = descriptor_iterator_player(runtime->descriptors, player);
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    num++;

  if (num < 2) {
    snprintf(buf, LBUF_SIZE, "%s has connected.",
             game_object_name(runtime->world->database, player));

    do_comconnect(&command->evaluation, player, d);

    if (is_dark(runtime->world->database, player)) {
      raw_broadcast(runtime->descriptors, OBJECT_FLAG_MONITOR,
                    "GAME: %s has DARK-connected.",
                    game_object_name(runtime->world->database, player));
    } else {
      raw_broadcast(runtime->descriptors, OBJECT_FLAG_MONITOR,
                    "GAME: %s has connected.",
                    game_object_name(runtime->world->database, player));
    }
  } else {
    snprintf(buf, LBUF_SIZE, "%s has reconnected.",
             game_object_name(runtime->world->database, player));
    raw_broadcast(runtime->descriptors, OBJECT_FLAG_MONITOR,
                  "GAME: %s has reconnected.",
                  game_object_name(runtime->world->database, player));
  }

  key = MSG_INV;
  if ((loc != NOTHING) && !(is_dark(runtime->world->database, player) &&
                            is_wizard(runtime->world->database, player)))
    key |= MSG_NBR | MSG_NBR_EXITS | MSG_LOC;

  temp = command->enactor;
  command->enactor = player;
  notify_checked(&command->evaluation, player, player, buf, key);
  free_lbuf(buf);
  if (is_suspect(runtime->world->database, player)) {
    send_channel(&command->evaluation, "Suspect", "%s has connected.",
                 game_object_name(runtime->world->database, player));
  }
  if (d->host_info & H_SUSPECT)
    send_channel(&command->evaluation, "Suspect",
                 "[Suspect site: %s] %s has connected.", d->addr,
                 game_object_name(runtime->world->database, player));
  dispatch_connection_event_scope(runtime, d, player, loc, LUA_EVENT_CONNECT,
                                  num >= 2, nullptr);
  time_str = ctime(&runtime->clock->now);
  time_str[strlen(time_str) - 1] = '\0';
  record_login(&command->evaluation, player, 1, time_str, d->addr, d->username);
  look_in(&descriptor_runtime(d)->background_command->evaluation, player,
          game_object_location(runtime->world->database, player), LK_SHOWEXIT);
  command->enactor = temp;
}

void descriptor_announce_disconnect(DbRef player, Descriptor *d,
                                    const char *reason) {
  CommandRuntime *runtime = descriptor_runtime(d);
  CommandContext *command = runtime->background_command;
  DbRef loc, temp;
  int num, key;
  char *buf;
  Descriptor *dtemp;
  DescriptorIterator iterator =
      descriptor_iterator_player(runtime->descriptors, player);

  if (is_suspect(runtime->world->database, player)) {
    send_channel(&command->evaluation, "Suspect", "%s has disconnected.",
                 game_object_name(runtime->world->database, player));
  }
  if (d->host_info & H_SUSPECT) {
    send_channel(&command->evaluation, "Suspect",
                 "[Suspect site: %s] %s has disconnected.", d->addr,
                 game_object_name(runtime->world->database, d->player));
  }
  loc = game_object_location(runtime->world->database, player);
  num = 0;
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    num++;

  temp = command->enactor;
  command->enactor = player;

  if (num == 0) {
    buf = alloc_mbuf("descriptor_announce_disconnect.only");

    snprintf(buf, MBUF_SIZE, "%s has disconnected.",
             game_object_name(runtime->world->database, player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(runtime->world->database, player) &&
                              is_wizard(runtime->world->database, player)))
      key |= MSG_NBR | MSG_NBR_EXITS | MSG_LOC;
    notify_checked(&command->evaluation, player, player, buf, key);
    free_mbuf(buf);

    do_comdisconnect(&command->evaluation, player);

    raw_broadcast(runtime->descriptors, OBJECT_FLAG_MONITOR,
                  "GAME: %s has disconnected.",
                  game_object_name(runtime->world->database, player));

    c_connected(runtime->world->database, player);
    dispatch_connection_event_scope(runtime, d, player, loc,
                                    LUA_EVENT_DISCONNECT, false, reason);
    if (d->is_autodark) {
      game_object_set_flag(runtime->world->database, d->player,
                           OBJECT_FLAG_DARK, false);
      d->is_autodark = false;
    }

  } else {
    buf = alloc_mbuf("descriptor_announce_disconnect.partial");
    snprintf(buf, MBUF_SIZE, "%s has partially disconnected.",
             game_object_name(runtime->world->database, player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(runtime->world->database, player) &&
                              is_wizard(runtime->world->database, player)))
      key |= MSG_NBR | MSG_NBR_EXITS | MSG_LOC;
    notify_checked(&command->evaluation, player, player, buf, key);
    raw_broadcast(runtime->descriptors, OBJECT_FLAG_MONITOR,
                  "GAME: %s has partially disconnected.",
                  game_object_name(runtime->world->database, player));
    free_mbuf(buf);
  }

  command->enactor = temp;
}

int boot_off(DescriptorRegistry *descriptors, DbRef player,
             const char *message) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_player(descriptors, player);
  int count;

  count = 0;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (message && *message) {
      descriptor_queue_string(d, message);
      descriptor_queue_string(d, "\r\n");
    }
    descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_BOOT);
    count++;
  }
  return count;
}

int boot_by_port(DescriptorRegistry *descriptors, int port, int no_god,
                 char *message) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_all(descriptors);
  int count;

  count = 0;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (d->is_dead)
      continue;
    if ((d->descriptor == port) &&
        (!no_god ||
         !is_god(descriptor_runtime(d)->world->database, d->player))) {
      if (message && *message) {
        descriptor_queue_string(d, message);
        descriptor_queue_string(d, "\r\n");
      }
      descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_BOOT);
      count++;
    }
  }
  return count;
}

/*
 * ---------------------------------------------------------------------------
 * * descriptor_reload: Reload parts of net descriptor that are based on db
 * info.
 */

void descriptor_reload(GameDatabase *database,
                       const ServerConfiguration *configuration,
                       DescriptorRegistry *descriptors, DbRef player) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_player(descriptors, player);
  char *buf;
  Flag aflags;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    buf = attribute_get(database, player, A_TIMEOUT, &aflags);
    if (buf) {
      d->timeout = clamped_atoi(buf);
      if (d->timeout <= 0)
        d->timeout = configuration->idle_timeout;
    }
    free_lbuf(buf);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fetch_idle, fetch_connect: Return smallest idle time/largest connec time
 * * for a player (or -1 if not logged in)
 */

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
  StringCopyTrunc(cbuff, name, 16);
  cbuff[16] = '\0';
  return cbuff;
}

static void dump_users(Descriptor *e, char *match) {
  CommandRuntime *runtime = descriptor_runtime(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(runtime->descriptors);
  int count;
  char *buf, *fp, *sp, flist[4], slist[4];

  while (match && *match && isspace((unsigned char)*match))
    match++;
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

    fp = flist;
    sp = slist;
    if (is_hidden(runtime->world->database, d->player)) {
      if (d->is_autodark)
        *fp++ = 'd';
      else if (is_dark(runtime->world->database, d->player))
        *fp++ = 'D';
    }
    if (is_suspect(runtime->world->database, d->player))
      *fp++ = '+';
    if (d->host_info & H_FORBIDDEN)
      *sp++ = 'F';
    if (d->host_info & H_SUSPECT)
      *sp++ = '+';
    *fp = '\0';
    *sp = '\0';

    snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%-3s#%6ld %7d %-25s\r\n",
             trimmed_name(runtime->world->database, d->player),
             time_format_1(runtime->clock->now - d->connected_at),
             time_format_2(runtime->clock->now - d->last_time), flist,
             game_object_location(runtime->world->database, d->player),
             d->command_count,
             (d->username[0] != '\0') ? tprintf("%s@%s", d->username, d->addr)
                                      : d->addr);
    descriptor_queue_string(e, buf);
  }
  snprintf(buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n",
           count, (count == 1) ? " " : "s ",
           *descriptor_runtime(e)->record_players,
           (runtime->world->configuration->max_players == -1)
               ? "no"
               : tprintf("%d", runtime->world->configuration->max_players));

  descriptor_queue_string(e, buf);

  free_lbuf(buf);
}

static void dump_sessions(Descriptor *e, char *match) {
  CommandRuntime *runtime = descriptor_runtime(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(runtime->descriptors);
  int count;
  char *buf;

  while (match && *match && isspace((unsigned char)*match))
    match++;
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

    snprintf(
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

  snprintf(buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n",
           count, (count == 1) ? " " : "s ",
           *descriptor_runtime(e)->record_players,
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
    notify(&invocation->context->evaluation, player,
           "@who is only available from an active connection.");
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

static int telnet_environment_view_compare(const void *left,
                                           const void *right) {
  const TelnetEnvironmentEntryView *left_entry = left;
  const TelnetEnvironmentEntryView *right_entry = right;
  size_t shared_size;
  int comparison;

  if (left_entry->kind != right_entry->kind)
    return left_entry->kind < right_entry->kind ? -1 : 1;
  shared_size = left_entry->name_size < right_entry->name_size
                    ? left_entry->name_size
                    : right_entry->name_size;
  comparison = memcmp(left_entry->name, right_entry->name, shared_size);
  if (comparison != 0)
    return comparison;
  if (left_entry->name_size == right_entry->name_size)
    return 0;
  return left_entry->name_size < right_entry->name_size ? -1 : 1;
}

static void telnet_append_escaped(char *buffer, char **position,
                                  const unsigned char *value, size_t size) {
  for (size_t index = 0; index < size; index++) {
    unsigned char byte = value[index];

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

static void dump_telnet_environment(EvaluationContext *evaluation, DbRef viewer,
                                    const Descriptor *descriptor) {
  constexpr size_t DISPLAY_CHUNK_SIZE = 3500;
  TelnetEnvironmentEntryView entries[64];
  size_t count = descriptor_telnet_environment_count(descriptor);
  char *buffer = alloc_lbuf("dump_telnet_environment");

  notify(evaluation, viewer, "  NEW-ENVIRON:");
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_new_environ_enabled ? "yes" : "no");
  if (count == 0) {
    notify(evaluation, viewer, "    Variables: (none)");
    free_lbuf(buffer);
    return;
  }
  if (count > sizeof(entries) / sizeof(entries[0]))
    count = sizeof(entries) / sizeof(entries[0]);
  for (size_t index = 0; index < count; index++)
    descriptor_telnet_environment_entry(descriptor, index, &entries[index]);
  qsort(entries, count, sizeof(entries[0]), telnet_environment_view_compare);
  notify(evaluation, viewer, "    Variables:");
  for (size_t index = 0; index < count; index++) {
    TelnetEnvironmentEntryView *entry = &entries[index];
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
      telnet_append_escaped(buffer, &position, entry->value + value_position,
                            chunk_size);
      safe_chr('"', buffer, &position);
      *position = '\0';
      notify(evaluation, viewer, buffer);
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
  notify(evaluation, viewer, "  TTYPE / MTTS:");
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
  notify(evaluation, viewer, "  NAWS:");
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_naws_enabled ? "yes" : "no");
  notify_printf(evaluation, viewer, "    Window size: %dx%d",
                descriptor->terminal_width, descriptor->terminal_height);
  notify(evaluation, viewer, "  CHARSET:");
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_charset_enabled ? "yes" : "no");
  notify_printf(evaluation, viewer, "    Encoding: %s",
                descriptor->is_charset_utf8 ? "UTF-8" : "unsupported");
  notify_printf(evaluation, viewer, "    Request pending: %s",
                descriptor->is_charset_request_pending ? "yes" : "no");
  dump_telnet_environment(evaluation, viewer, descriptor);
  notify(evaluation, viewer, "  GMCP:");
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_gmcp_enabled ? "yes" : "no");
  notify(evaluation, viewer, "  MSSP:");
  notify_printf(evaluation, viewer, "    Negotiated: %s",
                descriptor->is_mssp_enabled ? "yes" : "no");
  notify(evaluation, viewer, "  MCCP2:");
  notify_printf(evaluation, viewer, "    Compression: %s",
                descriptor->is_mccp_enabled ? "active" : "inactive");
  notify(evaluation, viewer, "  ECHO:");
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
    notify(evaluation, invocation->player, "Usage: @telnet <player>");
    return;
  }
  target = lookup_player(invocation->context->world, invocation->player,
                         invocation->first, 0);
  if (target == NOTHING) {
    notify(evaluation, invocation->player, "No such player.");
    return;
  }
  iterator =
      descriptor_iterator_player(evaluation->runtime->descriptors, target);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    dump_telnet_descriptor(evaluation, invocation->player, descriptor);
    count++;
  }
  if (count == 0)
    notify(evaluation, invocation->player, "That player is not connected.");
}

void do_color(CommandInvocation *invocation) {
  Descriptor *descriptor = invocation->context->descriptor;
  const char *mode = invocation->first;
  TerminalColorDepth requested;

  if (descriptor == nullptr || descriptor->player != invocation->player) {
    notify(&invocation->context->evaluation, invocation->player,
           "color is only available from an active connection.");
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
    if (!strcasecmp(mode, "off"))
      requested = TERMINAL_COLOR_NONE;
    else if (!strcmp(mode, "16"))
      requested = TERMINAL_COLOR_ANSI_16;
    else if (!strcmp(mode, "256"))
      requested = TERMINAL_COLOR_ANSI_256;
    else if (!strcasecmp(mode, "truecolor"))
      requested = TERMINAL_COLOR_TRUECOLOR;
    else {
      notify(&invocation->context->evaluation, invocation->player,
             "Use color auto, off, 16, 256, or truecolor.");
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
    notify(&invocation->context->evaluation, player,
           "@session is only available from an active connection.");
    return;
  }
  dump_sessions(descriptor, match);
}

void do_quit(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  Descriptor *descriptor = invocation->context->descriptor;

  if (descriptor == nullptr || descriptor->player != player) {
    notify(&invocation->context->evaluation, player,
           "quit is only available from an active connection.");
    return;
  }
  descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_QUIT);
}

int descriptor_command(Descriptor *d, char *command) {
  CommandRuntime *runtime = descriptor_runtime(d);
  CommandContext context;

  if (!command_context_initialize(&context, runtime, descriptor_btech(d),
                                  descriptor_log(d), d->player, d->player, d,
                                  true))
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
int site_data_check(struct sockaddr_storage *saddr, int saddr_len,
                    SiteData *site_list) {
  SiteData *this;
  for (this = site_list; this; this = this->next) {
    if ((((struct sockaddr_in *)saddr)->sin_addr.s_addr & this->mask.s_addr) ==
        this->address.s_addr) {
      return this->flag;
    }
  }
  return 0;
}

/*
 * --------------------------------------------------------------------------
 * * list_sites: Display information in a site list
 */

#define S_SUSPECT 1
#define S_ACCESS 2

static const char *stat_string(int strtype, int flag) {
  const char *str;

  switch (strtype) {
  case S_SUSPECT:
    if (flag)
      str = "Suspected";
    else
      str = "Trusted";
    break;
  case S_ACCESS:
    switch (flag) {
    case H_FORBIDDEN:
      str = "Forbidden";
      break;
    case 0:
      str = "Unrestricted";
      break;
    default:
      str = "Strange";
    }
    break;
  default:
    str = "Strange";
  }
  return str;
}

static void list_sites(EvaluationContext *evaluation, DbRef player,
                       SiteData *site_list, const char *header_txt,
                       int stat_type) {
  char *buff, *buff1;
  const char *str;
  SiteData *this;

  buff = alloc_mbuf("list_sites.buff");
  buff1 = alloc_sbuf("list_sites.addr");
  snprintf(buff, MBUF_SIZE, "----- %s -----", header_txt);
  notify(evaluation, player, buff);
  notify(evaluation, player,
         "Address              Mask                 Status");
  for (this = site_list; this; this = this->next) {
    str = stat_string(stat_type, this->flag);
    StringCopy(buff1, inet_ntoa(this->mask));
    snprintf(buff, MBUF_SIZE, "%-20s %-20s %s", inet_ntoa(this->address), buff1,
             str);
    notify(evaluation, player, buff);
  }
  free_mbuf(buff);
  free_sbuf(buff1);
}

/*
 * ---------------------------------------------------------------------------
 * * list_siteinfo: List information about specially-marked sites.
 */

void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player) {
  list_sites(evaluation, player, access_control->access_sites, "Site Access",
             S_ACCESS);
  list_sites(evaluation, player, access_control->suspect_sites,
             "Suspected Sites", S_SUSPECT);
}

/*
 * ---------------------------------------------------------------------------
 * * make_ulist: Make a list of connected user numbers for the LWHO function.
 */

void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buff, char **bufc) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  char *cp;

  cp = *bufc;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (!is_wizard(database, player) && is_hidden(database, d->player))
      continue;
    if (cp != *bufc)
      safe_chr(' ', buff, bufc);
    safe_chr('#', buff, bufc);
    safe_str(tprintf("%ld", d->player), buff, bufc);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * find_connected_name: Resolve a playername from the list of connected
 * * players using prefix matching.  We only return a match if the prefix
 * * was unique.
 */

DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          char *name) {
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_connected(descriptors);
  DbRef found;

  found = NOTHING;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (is_good_obj(database, player) && !is_wizard(database, player) &&
        is_hidden(database, d->player))
      continue;
    if (!string_prefix(game_object_pure_name(database, d->player), name))
      continue;
    if ((found != NOTHING) && (found != d->player))
      return NOTHING;
    found = d->player;
  }
  return found;
}

void descriptor_run_command(Descriptor *d, char *command) {
  if (!is_wizard(descriptor_runtime(d)->world->database, d->player)) {
    if (d->quota <= 0) {
      descriptor_queue_string(d, "quota exceed, dropping command.\n");
      dprintk("aborting execution of %s for #%ld.", command, d->player);
      return;
    }
    d->quota--;
  }
  descriptor_command(d, command);
}
