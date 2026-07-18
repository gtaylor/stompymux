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

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/stringutil.h"

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
 * * update_quotas: Update timeslice quotas
 */

struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current) {
  int nslices;
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_all(descriptors);

  nslices = msec_diff(current, last) /
            (configuration->timeslice > 0 ? configuration->timeslice : 1);

  if (nslices > 0) {
    while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
      if (d->is_dead)
        continue;
      d->quota += configuration->cmd_quota_incr * nslices;
      if (d->quota > configuration->cmd_quota_max)
        d->quota = configuration->cmd_quota_max;
    }
  }
  return msec_add(last, nslices * configuration->timeslice);
}

/* raw_notify_raw: write a message to a player without the newline */

void raw_notify_raw(EvaluationContext *evaluation, DbRef player,
                    const char *msg, const char *append) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_player(evaluation->server->descriptors, player);

  if (!msg || !*msg)
    return;

  if (evaluation->is_piping && (player == evaluation->pipe_object)) {
    safe_str(msg, evaluation->pipe_next, &evaluation->pipe_cursor);
    if (append != nullptr)
      safe_str(append, evaluation->pipe_next, &evaluation->pipe_cursor);
    return;
  }

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
      descriptor_iterator_player(evaluation->server->descriptors, player);
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
      descriptor_iterator_player(evaluation->server->descriptors, player);

  if (evaluation->is_piping && (player == evaluation->pipe_object)) {
    safe_str("\r\n", evaluation->pipe_next, &evaluation->pipe_cursor);
    return;
  }
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
    if ((game_object_flags(&descriptor_server(d)->database, d->player) &
         inflags) == inflags) {
      descriptor_queue_write(d, buff, (int)strnlen(buff, LBUF_SIZE - 1));
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
  char new[LBUF_SIZE];

  strncpy(new, s, LBUF_SIZE - 1);
  new[LBUF_SIZE - 1] = '\0';

  if (!is_ansi(&descriptor_server(d)->database, d->player) &&
      index(s, ESC_CHAR))
    strip_ansi_r(new, s, strlen(s));
  descriptor_queue_write(d, new, (int)strlen(new));
}

void descriptor_welcome(Descriptor *d) {
  FileCache *files = descriptor_server(d)->files;
  int connection_count = file_cache_connection_count(files);

  if (connection_count) {
    fcache_dump_conn(files, d, rand() % connection_count);
    return;
  }
  fcache_dump(files, d, FC_CONN);
}

void set_lastsite(Descriptor *d, char *lastsite) {
  MuxServer *server = descriptor_server(d);
  long i, j;
  char buf[LBUF_SIZE];

  if (d->player) {
    if (lastsite) {
      strncpy(buf, lastsite, LBUF_SIZE - 1);
      buf[LBUF_SIZE - 1] = '\0';
    } else {
      attribute_get_string(&server->database, buf, d->player, A_LASTSITE, &i,
                           &j);
    }
    attribute_add_raw(&server->database, d->player, A_LASTSITE, buf);
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

void announce_connect(DbRef player, Descriptor *d) {
  MuxServer *server = descriptor_server(d);
  const ServerConfiguration *configuration = server->configuration;
  CommandContext *command = &server->background_command;
  DbRef loc, aowner, temp;
  DbRef zone, obj;

  long aflags;
  int num, key, count;
  char *buf, *time_str;
  Descriptor *dtemp;
  DescriptorIterator iterator;

  descriptor_queue_string(d, "Connected.\n\n");

  count = 0;
  iterator = descriptor_iterator_connected(server->descriptors);
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    count++;

  if (server->record_players < count)
    server->record_players = count;

  buf = attribute_parent_get(&server->database, player, A_TIMEOUT, &aowner,
                             &aflags);
  if (buf) {
    d->timeout = clamped_atoi(buf);
    if (d->timeout <= 0)
      d->timeout = configuration->idle_timeout;
  }
  free_lbuf(buf);

  loc = game_object_location(&server->database, player);
  s_connected(&server->database, player);

  if (is_wizard(&server->database, player)) {
    if (!configuration->is_login_enabled) {
      raw_notify(&command->evaluation, player, "*** Logins are disabled.");
    }
  }
  buf = alloc_lbuf("announce_connect");
  num = 0;
  iterator = descriptor_iterator_player(server->descriptors, player);
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    num++;

  if (num < 2) {
    snprintf(buf, LBUF_SIZE, "%s has connected.",
             game_object_name(&server->database, player));

    if (configuration->have_comsys)
      do_comconnect(&command->evaluation, player, d);

    if (is_dark(&server->database, player)) {
      raw_broadcast(server->descriptors, MONITOR,
                    "GAME: %s has DARK-connected.",
                    game_object_name(&server->database, player));
    } else {
      raw_broadcast(server->descriptors, MONITOR, "GAME: %s has connected.",
                    game_object_name(&server->database, player));
    }
  } else {
    snprintf(buf, LBUF_SIZE, "%s has reconnected.",
             game_object_name(&server->database, player));
    raw_broadcast(server->descriptors, MONITOR, "GAME: %s has reconnected.",
                  game_object_name(&server->database, player));
  }

  key = MSG_INV;
  if ((loc != NOTHING) && !(is_dark(&server->database, player) &&
                            is_wizard(&server->database, player)))
    key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);

  temp = command->enactor;
  command->enactor = player;
  notify_checked(&command->evaluation, player, player, buf, key);
  free_lbuf(buf);
  if (is_suspect(&server->database, player)) {
    send_channel(&command->evaluation, "Suspect", "%s has connected.",
                 game_object_name(&server->database, player));
  }
  if (d->host_info & H_SUSPECT)
    send_channel(&command->evaluation, "Suspect",
                 "[Suspect site: %s] %s has connected.", d->addr,
                 game_object_name(&server->database, player));
  buf = attribute_parent_get(&server->database, player, A_ACONNECT, &aowner,
                             &aflags);
  if (buf)
    wait_que(server->commands, player, player, 0, NOTHING, 0, buf,
             (char **)nullptr, 0, nullptr);
  free_lbuf(buf);
  if (configuration->master_room != NOTHING) {
    buf = attribute_parent_get(&server->database, configuration->master_room,
                               A_ACONNECT, &aowner, &aflags);
    if (buf)
      wait_que(server->commands, configuration->master_room, player, 0, NOTHING,
               0, buf, (char **)nullptr, 0, nullptr);
    free_lbuf(buf);
    DOLIST(
        &server->database, obj,
        game_object_contents(&server->database, configuration->master_room)) {
      buf = attribute_parent_get(&server->database, obj, A_ACONNECT, &aowner,
                                 &aflags);
      if (buf) {
        wait_que(server->commands, obj, player, 0, NOTHING, 0, buf,
                 (char **)nullptr, 0, nullptr);
      }
      free_lbuf(buf);
    }
  }
  /*
   * do the zone of the player's location's possible aconnect
   */
  if (configuration->have_zones &&
      ((zone = game_object_zone(&server->database, loc)) != NOTHING)) {
    switch (typeof_obj(&server->database, zone)) {
    case TYPE_THING:
      buf = attribute_parent_get(&server->database, zone, A_ACONNECT, &aowner,
                                 &aflags);
      if (buf) {
        wait_que(server->commands, zone, player, 0, NOTHING, 0, buf,
                 (char **)nullptr, 0, nullptr);
      }
      free_lbuf(buf);
      break;
    case TYPE_ROOM:
      /*
       * check every object in the room for a connect * * *
       * action
       */
      DOLIST(&server->database, obj,
             game_object_contents(&server->database, zone)) {
        buf = attribute_parent_get(&server->database, obj, A_ACONNECT, &aowner,
                                   &aflags);
        if (buf) {
          wait_que(server->commands, obj, player, 0, NOTHING, 0, buf,
                   (char **)nullptr, 0, nullptr);
        }
        free_lbuf(buf);
      }
      break;
    default:
      log_text(tprintf("Invalid zone #%ld for %s(#%ld) has bad type %d", zone,
                       game_object_name(&server->database, player), player,
                       typeof_obj(&server->database, zone)));
    }
  }
  time_str = ctime(&server->clock.now);
  time_str[strlen(time_str) - 1] = '\0';
  record_login(&command->evaluation, player, 1, time_str, d->addr, d->username);
  look_in(&descriptor_server(d)->background_command.evaluation, player,
          game_object_location(&server->database, player), LK_SHOWEXIT);
  command->enactor = temp;
}

void descriptor_announce_disconnect(DbRef player, Descriptor *d,
                                    const char *reason) {
  MuxServer *server = descriptor_server(d);
  const ServerConfiguration *configuration = server->configuration;
  CommandContext *command = &server->background_command;
  DbRef loc, aowner, temp, zone, obj;
  int num, key;
  long aflags;
  char *buf, *atr_temp;
  Descriptor *dtemp;
  DescriptorIterator iterator =
      descriptor_iterator_player(server->descriptors, player);
  char *argv[1];

  if (is_suspect(&server->database, player)) {
    send_channel(&command->evaluation, "Suspect", "%s has disconnected.",
                 game_object_name(&server->database, player));
  }
  if (d->host_info & H_SUSPECT) {
    send_channel(&command->evaluation, "Suspect",
                 "[Suspect site: %s] %s has disconnected.", d->addr,
                 game_object_name(&server->database, d->player));
  }
  loc = game_object_location(&server->database, player);
  num = 0;
  while ((dtemp = descriptor_iterator_next(&iterator)) != nullptr)
    num++;

  temp = command->enactor;
  command->enactor = player;

  if (num == 0) {
    buf = alloc_mbuf("descriptor_announce_disconnect.only");

    snprintf(buf, MBUF_SIZE, "%s has disconnected.",
             game_object_name(&server->database, player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(&server->database, player) &&
                              is_wizard(&server->database, player)))
      key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);
    notify_checked(&command->evaluation, player, player, buf, key);
    free_mbuf(buf);

    if (configuration->have_comsys)
      do_comdisconnect(&command->evaluation, player);

    raw_broadcast(server->descriptors, MONITOR, "GAME: %s has disconnected.",
                  game_object_name(&server->database, player));

    /* wait_que()'s argument array isn't const-correct; reason is only
       read as command-queue text here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    argv[0] = (char *)reason;
#pragma clang diagnostic pop
    c_connected(&server->database, player);

    atr_temp = attribute_parent_get(&server->database, player, A_ADISCONNECT,
                                    &aowner, &aflags);
    if (atr_temp && *atr_temp)
      wait_que(server->commands, player, player, 0, NOTHING, 0, atr_temp, argv,
               1, nullptr);
    free_lbuf(atr_temp);
    if (configuration->master_room != NOTHING) {
      atr_temp =
          attribute_parent_get(&server->database, configuration->master_room,
                               A_ADISCONNECT, &aowner, &aflags);
      if (atr_temp)
        wait_que(server->commands, configuration->master_room, player, 0,
                 NOTHING, 0, atr_temp, (char **)nullptr, 0, nullptr);
      free_lbuf(atr_temp);
      DOLIST(
          &server->database, obj,
          game_object_contents(&server->database, configuration->master_room)) {
        atr_temp = attribute_parent_get(&server->database, obj, A_ADISCONNECT,
                                        &aowner, &aflags);
        if (atr_temp) {
          wait_que(server->commands, obj, player, 0, NOTHING, 0, atr_temp,
                   (char **)nullptr, 0, nullptr);
        }
        free_lbuf(atr_temp);
      }
    }
    /*
     * do the zone of the player's location's possible * * *
     * adisconnect
     */
    if (configuration->have_zones &&
        ((zone = game_object_zone(&server->database, loc)) != NOTHING)) {
      switch (typeof_obj(&server->database, zone)) {
      case TYPE_THING:
        atr_temp = attribute_parent_get(&server->database, zone, A_ADISCONNECT,
                                        &aowner, &aflags);
        if (atr_temp) {
          wait_que(server->commands, zone, player, 0, NOTHING, 0, atr_temp,
                   (char **)nullptr, 0, nullptr);
        }
        free_lbuf(atr_temp);
        break;
      case TYPE_ROOM:
        /*
         * check every object in the room for a * * *
         * connect action
         */
        DOLIST(&server->database, obj,
               game_object_contents(&server->database, zone)) {
          atr_temp = attribute_parent_get(&server->database, obj, A_ADISCONNECT,
                                          &aowner, &aflags);
          if (atr_temp) {
            wait_que(server->commands, obj, player, 0, NOTHING, 0, atr_temp,
                     (char **)nullptr, 0, nullptr);
          }
          free_lbuf(atr_temp);
        }
        break;
      default:
        log_text(tprintf("Invalid zone #%ld for %s(#%ld) has bad type %d", zone,
                         game_object_name(&server->database, player), player,
                         typeof_obj(&server->database, zone)));
      }
    }
    if (d->is_autodark) {
      game_object_set_flags(&server->database, d->player,
                            game_object_flags(&server->database, d->player) &
                                ~DARK);
      d->is_autodark = false;
    }

  } else {
    buf = alloc_mbuf("descriptor_announce_disconnect.partial");
    snprintf(buf, MBUF_SIZE, "%s has partially disconnected.",
             game_object_name(&server->database, player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(&server->database, player) &&
                              is_wizard(&server->database, player)))
      key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);
    notify_checked(&command->evaluation, player, player, buf, key);
    raw_broadcast(server->descriptors, MONITOR,
                  "GAME: %s has partially disconnected.",
                  game_object_name(&server->database, player));
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
        (!no_god || !is_god(&descriptor_server(d)->database, d->player))) {
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
  DbRef aowner;
  Flag aflags;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    buf = attribute_parent_get(database, player, A_TIMEOUT, &aowner, &aflags);
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

  if (strlen(game_object_name(database, player)) <= 16)
    return game_object_name(database, player);
  StringCopyTrunc(cbuff, game_object_name(database, player), 16);
  cbuff[16] = '\0';
  return cbuff;
}

static void dump_users(Descriptor *e, char *match) {
  MuxServer *server = descriptor_server(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(server->descriptors);
  int count;
  char *buf, *fp, *sp, flist[4], slist[4];
  DbRef room_it;

  while (match && *match && isspace(*match))
    match++;
  if (!match || !*match)
    match = nullptr;

  buf = alloc_lbuf("dump_users");
  descriptor_queue_string(e, "Player Name         On For  Idle ");
  descriptor_queue_string(e, "     Room    Cmds Host\r\n");
  count = 0;
  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (match &&
        !(string_prefix(game_object_name(&server->database, d->player), match)))
      continue;
    count++;

    fp = flist;
    sp = slist;
    if (is_hidden(&server->database, d->player)) {
      if (d->is_autodark)
        *fp++ = 'd';
      else if (is_dark(&server->database, d->player))
        *fp++ = 'D';
    }
    if (!is_findable(&server->database, d->player)) {
      *fp++ = 'U';
    } else {
      room_it = where_room(&server->database, server->configuration, d->player);
      if (is_good_obj(&server->database, room_it)) {
        if (is_hideout(&server->database, room_it))
          *fp++ = 'u';
      } else {
        *fp++ = 'u';
      }
    }

    if (is_suspect(&server->database, d->player))
      *fp++ = '+';
    if (d->host_info & H_FORBIDDEN)
      *sp++ = 'F';
    if (d->host_info & H_SUSPECT)
      *sp++ = '+';
    *fp = '\0';
    *sp = '\0';

    snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%-3s#%6ld %7d %-25s\r\n",
             trimmed_name(&server->database, d->player),
             time_format_1(server->clock.now - d->connected_at),
             time_format_2(server->clock.now - d->last_time), flist,
             game_object_location(&server->database, d->player),
             d->command_count,
             (d->username[0] != '\0') ? tprintf("%s@%s", d->username, d->addr)
                                      : d->addr);
    descriptor_queue_string(e, buf);
  }
  snprintf(buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n",
           count, (count == 1) ? " " : "s ",
           descriptor_server(e)->record_players,
           (server->configuration->max_players == -1)
               ? "no"
               : tprintf("%d", server->configuration->max_players));

  descriptor_queue_string(e, buf);

  free_lbuf(buf);
}

static void dump_sessions(Descriptor *e, char *match) {
  MuxServer *server = descriptor_server(e);
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_connected(server->descriptors);
  int count;
  char *buf;

  while (match && *match && isspace(*match))
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
        !string_prefix(game_object_name(&server->database, d->player), match))
      continue;
    count++;

    snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%5d%5d%6d%10d%6d%6d%10d\r\n",
             trimmed_name(&server->database, d->player),
             time_format_1(server->clock.now - d->connected_at),
             time_format_2((server->clock.now - d->last_time) > HIDDEN_IDLESECS
                               ? (server->clock.now - d->last_time)
                               : 0),
             d->descriptor, d->input_size, d->input_lost, d->input_tot,
             d->output_size, d->output_lost, d->output_tot);
    descriptor_queue_string(e, buf);
  }

  snprintf(buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n",
           count, (count == 1) ? " " : "s ",
           descriptor_server(e)->record_players,
           (server->configuration->max_players == -1)
               ? "no"
               : tprintf("%d", server->configuration->max_players));
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
  MuxServer *server = descriptor_server(d);
  CommandContext context;
  CommandContext *previous_context = server->btech.command_context;

  if (!command_context_initialize(&context, server, d->player, d->player, d,
                                  true))
    return 0;
  btech_context_set_command(&server->btech, &context);
  context.debug_command = "< descriptor_command >";

  /* The IDLE command is used to keep players behind badly configured NATs
     alive. This does not increment command count or idle time and is a
     good alternative to a lot of the current anti-disconnectors out there.
  */
  if (!strcasecmp(command, "IDLE") && d->is_connected) {
    context.debug_command = "idle";
    btech_context_set_command(&server->btech, previous_context);
    command_context_destroy(&context);
    return 1;
  }

  d->last_time = server->clock.now;
  d->command_count++;
  process_command(&context, command, (char **)nullptr, 0);
  btech_context_set_command(&server->btech, previous_context);
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
    if (!string_prefix(game_object_name(database, d->player), name))
      continue;
    if ((found != NOTHING) && (found != d->player))
      return NOTHING;
    found = d->player;
  }
  return found;
}

void descriptor_run_command(Descriptor *d, char *command) {
  if (!is_wizard(&descriptor_server(d)->database, d->player)) {
    if (d->quota <= 0) {
      descriptor_queue_string(d, "quota exceed, dropping command.\n");
      dprintk("aborting execution of %s for #%ld.", command, d->player);
      return;
    }
    d->quota--;
  }
  descriptor_command(d, command);
}
