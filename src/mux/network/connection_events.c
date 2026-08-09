#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * netcommon.c
 */

/*
 * This file contains routines used by the networking code that do not
 * depend on the implementation of the networking code.  The network-specific
 * portions of the descriptor data structure are not used.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_context.h"
#include "mux/commands/look.h"
#include "mux/communication/comsys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/connection_events.h"
#include "mux/network/descriptor.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/player_account.h"
#include "mux/server/file_cache.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

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
  char buf[LBUF_SIZE];

  if (d->player) {
    if (lastsite) {
      strncpy(buf, lastsite, LBUF_SIZE - 1);
      buf[LBUF_SIZE - 1] = '\0';
    } else {
      snprintf(buf, sizeof(buf), "%s",
               player_account_last_site(runtime->world->database, d->player));
    }
    player_account_last_site_set(runtime->world->database, d->player, buf);
  }
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
  descriptor_queue_string(d, "Connected.\n\n");

  int count = 0;
  DescriptorIterator iterator =
      descriptor_iterator_connected(runtime->descriptors);
  while (descriptor_iterator_next(&iterator) != nullptr)
    count++;

  if (*runtime->record_players < count)
    *runtime->record_players = count;

  DbRef loc = game_object_location(runtime->world->database, player);
  s_connected(runtime->world->database, player);

  if (is_wizard(runtime->world->database, player)) {
    if (!configuration->is_login_enabled) {
      raw_notify(&command->evaluation, player, "*** Logins are disabled.");
    }
  }
  char *buf = alloc_lbuf("announce_connect");
  int num = 0;
  iterator = descriptor_iterator_player(runtime->descriptors, player);
  while (descriptor_iterator_next(&iterator) != nullptr)
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

  int key = MSG_INV;
  if ((loc != NOTHING) && !(is_dark(runtime->world->database, player) &&
                            is_wizard(runtime->world->database, player)))
    key |= MSG_NBR | MSG_NBR_EXITS | MSG_LOC;

  DbRef temp = command->enactor;
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
  record_login(&command->evaluation, player, true, runtime->clock->now, d->addr,
               d->username);
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
  while (descriptor_iterator_next(&iterator) != nullptr)
    num++;

  temp = command->enactor;
  command->enactor = player;

  if (num == 0) {
    char buf[MBUF_SIZE];

    snprintf(buf, MBUF_SIZE, "%s has disconnected.",
             game_object_name(runtime->world->database, player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(runtime->world->database, player) &&
                              is_wizard(runtime->world->database, player)))
      key |= MSG_NBR | MSG_NBR_EXITS | MSG_LOC;
    notify_checked(&command->evaluation, player, player, buf, key);

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
    char buf[MBUF_SIZE];
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
 * * fetch_idle, fetch_connect: Return smallest idle time/largest connec time
 * * for a player (or -1 if not logged in)
 */
