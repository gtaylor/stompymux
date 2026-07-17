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
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/stringutil.h"

enum {
  CMD_QUIT = 1,
  CMD_WHO = 2,
  CMD_DOING = 3,
  CMD_PREFIX = 5,
  CMD_SUFFIX = 6,
  CMD_SESSION = 8,
  CMD_MASK = 0xff,
  CMD_NOxFIX = 0x100,
};

/*
 * ---------------------------------------------------------------------------
 * * make_portlist: Make a list of ports for PORTS().
 */

void make_portlist(DbRef player, DbRef target, char *buff, char **bufc) {
  Descriptor *d;
  int i = 0;

  DESC_ITER_CONN(d) {
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

struct timeval update_quotas(struct timeval last, struct timeval current) {
  int nslices;
  Descriptor *d;

  nslices = msec_diff(current, last) /
            (mudconf.timeslice > 0 ? mudconf.timeslice : 1);

  if (nslices > 0) {
    DESC_ITER_ALL(d) {
      d->quota += mudconf.cmd_quota_incr * nslices;
      if (d->quota > mudconf.cmd_quota_max)
        d->quota = mudconf.cmd_quota_max;
    }
  }
  return msec_add(last, nslices * mudconf.timeslice);
}

/* raw_notify_raw: write a message to a player without the newline */

void raw_notify_raw(DbRef player, const char *msg, const char *append) {
  Descriptor *d;

  if (!msg || !*msg)
    return;

  if (mudstate.inpipe && (player == mudstate.poutobj)) {
    safe_str(msg, mudstate.poutnew, &mudstate.poutbufc);
    if (append != nullptr)
      safe_str(append, mudstate.poutnew, &mudstate.poutbufc);
    return;
  }

  if (!is_connected(player))
    return;

  DESC_ITER_PLAYER(player, d) {
    descriptor_queue_string(d, msg);
    if (append != nullptr)
      descriptor_queue_write(d, append, (int)strlen(append));
  }
}

/* raw_notify: write a message to a player */
void raw_notify(DbRef player, const char *msg) {
  raw_notify_raw(player, msg, "\r\n");
}

void notify_printf(DbRef player, const char *format, ...) {
  Descriptor *d;
  char buffer[LBUF_SIZE];
  va_list ap;
  memset(buffer, 0, LBUF_SIZE);

  va_start(ap, format);

  vsnprintf(buffer, LBUF_SIZE - 1, format, ap);
  va_end(ap);

  strncat(buffer, "\r\n", LBUF_SIZE - 1);
  buffer[LBUF_SIZE - 1] = '\0';

  DESC_ITER_PLAYER(player, d) { descriptor_queue_string(d, buffer); }
}

void raw_notify_newline(DbRef player) {
  Descriptor *d;

  if (mudstate.inpipe && (player == mudstate.poutobj)) {
    safe_str("\r\n", mudstate.poutnew, &mudstate.poutbufc);
    return;
  }
  if (!is_connected(player))
    return;

  DESC_ITER_PLAYER(player, d) { descriptor_queue_write(d, "\r\n", 2); }
}

/*
 * ---------------------------------------------------------------------------
 * * raw_broadcast: Send message to players who have indicated flags
 */

void raw_broadcast(int inflags, const char *template, ...) {
  char buff[LBUF_SIZE];
  Descriptor *d;
  va_list ap;

  if (!template || !*template)
    return;

  va_start(ap, template);
  vsnprintf(buff, LBUF_SIZE, template, ap);
  buff[LBUF_SIZE - 1] = '\0';

  DESC_ITER_CONN(d) {
    if ((obj_flags(d->player) & inflags) == inflags) {
      descriptor_queue_write(d, buff, (int)strnlen(buff, LBUF_SIZE - 1));
      descriptor_queue_write(d, "\r\n", 2);
    }
  }
  flush_sockets();
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

  if (!is_ansi(d->player) && index(s, ESC_CHAR))
    strip_ansi_r(new, s, strlen(s));
  descriptor_queue_write(d, new, (int)strlen(new));
}

extern int fcache_conn_c;

void descriptor_welcome(Descriptor *d) {
  if (fcache_conn_c) {
    fcache_dump_conn(d, rand() % fcache_conn_c);
    return;
  }
  fcache_dump(d, FC_CONN);
}

void set_lastsite(Descriptor *d, char *lastsite) {
  long i, j;
  char buf[LBUF_SIZE];

  if (d->player) {
    if (lastsite) {
      strncpy(buf, lastsite, LBUF_SIZE - 1);
      buf[LBUF_SIZE - 1] = '\0';
    } else {
      attribute_get_string(buf, d->player, A_LASTSITE, &i, &j);
    }
    attribute_add_raw(d->player, A_LASTSITE, buf);
  }
}

static void set_userstring(char **userstring, const char *command) {
  while (*command && isascii(*command) && isspace(*command))
    command++;
  if (!*command) {
    if (*userstring != nullptr) {
      free_lbuf(*userstring);
      *userstring = nullptr;
    }
  } else {
    if (*userstring == nullptr) {
      *userstring = alloc_lbuf("set_userstring");
    }
    snprintf(*userstring, LBUF_SIZE - 1, "%s\r\n", command);
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

void descriptor_hash_add(Descriptor *);

void announce_connect(DbRef player, Descriptor *d) {
  DbRef loc, aowner, temp;
  DbRef zone, obj;

  long aflags;
  int num, key, count;
  char *buf, *time_str;
  Descriptor *dtemp;

  descriptor_queue_string(d, "Connected.\n\n");

  descriptor_hash_add(d);

  count = 0;
  DESC_ITER_CONN(dtemp)
  count++;

  if (mudstate.record_players < count)
    mudstate.record_players = count;

  buf = attribute_parent_get(player, A_TIMEOUT, &aowner, &aflags);
  if (buf) {
    d->timeout = clamped_atoi(buf);
    if (d->timeout <= 0)
      d->timeout = mudconf.idle_timeout;
  }
  free_lbuf(buf);

  loc = obj_location(player);
  s_connected(player);

  if (is_wizard(player)) {
    if (!(mudconf.control_flags & CF_LOGIN)) {
      raw_notify(player, "*** Logins are disabled.");
    }
  }
  buf = attribute_get(player, A_LPAGE, &aowner, &aflags);
  if (buf && *buf) {
    raw_notify(
        player,
        "Your PAGE LOCK is set. You may be unable to receive some pages.");
  }
  num = 0;
  DESC_ITER_PLAYER(player, dtemp) num++;

  if (num < 2) {
    snprintf(buf, LBUF_SIZE, "%s has connected.", Name(player));

    if (mudconf.have_comsys)
      do_comconnect(player, d);

    if (is_dark(player)) {
      raw_broadcast(MONITOR, "GAME: %s has DARK-connected.", Name(player));
    } else {
      raw_broadcast(MONITOR, "GAME: %s has connected.", Name(player));
    }
  } else {
    snprintf(buf, LBUF_SIZE, "%s has reconnected.", Name(player));
    raw_broadcast(MONITOR, "GAME: %s has reconnected.", Name(player));
  }

  key = MSG_INV;
  if ((loc != NOTHING) && !(is_dark(player) && is_wizard(player)))
    key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);

  temp = mudstate.curr_enactor;
  mudstate.curr_enactor = player;
  notify_checked(player, player, buf, key);
  free_lbuf(buf);
  if (is_suspect(player)) {
    send_channel("Suspect", "%s has connected.", Name(player));
  }
  if (d->host_info & H_SUSPECT)
    send_channel("Suspect", "[Suspect site: %s] %s has connected.", d->addr,
                 Name(player));
  buf = attribute_parent_get(player, A_ACONNECT, &aowner, &aflags);
  if (buf)
    wait_que(player, player, 0, NOTHING, 0, buf, (char **)nullptr, 0, nullptr);
  free_lbuf(buf);
  if (mudconf.master_room != NOTHING) {
    buf =
        attribute_parent_get(mudconf.master_room, A_ACONNECT, &aowner, &aflags);
    if (buf)
      wait_que(mudconf.master_room, player, 0, NOTHING, 0, buf,
               (char **)nullptr, 0, nullptr);
    free_lbuf(buf);
    DOLIST(obj, obj_contents(mudconf.master_room)) {
      buf = attribute_parent_get(obj, A_ACONNECT, &aowner, &aflags);
      if (buf) {
        wait_que(obj, player, 0, NOTHING, 0, buf, (char **)nullptr, 0, nullptr);
      }
      free_lbuf(buf);
    }
  }
  /*
   * do the zone of the player's location's possible aconnect
   */
  if (mudconf.have_zones && ((zone = obj_zone(loc)) != NOTHING)) {
    switch (typeof_obj(zone)) {
    case TYPE_THING:
      buf = attribute_parent_get(zone, A_ACONNECT, &aowner, &aflags);
      if (buf) {
        wait_que(zone, player, 0, NOTHING, 0, buf, (char **)nullptr, 0,
                 nullptr);
      }
      free_lbuf(buf);
      break;
    case TYPE_ROOM:
      /*
       * check every object in the room for a connect * * *
       * action
       */
      DOLIST(obj, obj_contents(zone)) {
        buf = attribute_parent_get(obj, A_ACONNECT, &aowner, &aflags);
        if (buf) {
          wait_que(obj, player, 0, NOTHING, 0, buf, (char **)nullptr, 0,
                   nullptr);
        }
        free_lbuf(buf);
      }
      break;
    default:
      log_text(tprintf("Invalid zone #%ld for %s(#%ld) has bad type %d", zone,
                       Name(player), player, typeof_obj(zone)));
    }
  }
  time_str = ctime(&mudstate.now);
  time_str[strlen(time_str) - 1] = '\0';
  record_login(player, 1, time_str, d->addr, d->username);
  look_in(player, obj_location(player), LK_SHOWEXIT);
  mudstate.curr_enactor = temp;
}

void descriptor_announce_disconnect(DbRef player, Descriptor *d,
                                    const char *reason) {
  DbRef loc, aowner, temp, zone, obj;
  int num, key;
  long aflags;
  char *buf, *atr_temp;
  Descriptor *dtemp;
  char *argv[1];

  if (is_suspect(player)) {
    send_channel("Suspect", "%s has disconnected.", Name(player));
  }
  if (d->host_info & H_SUSPECT) {
    send_channel("Suspect", "[Suspect site: %s] %s has disconnected.", d->addr,
                 Name(d->player));
  }
  loc = obj_location(player);
  num = 0;
  DESC_ITER_PLAYER(player, dtemp) num++;

  temp = mudstate.curr_enactor;
  mudstate.curr_enactor = player;

  if (num < 2) {
    buf = alloc_mbuf("descriptor_announce_disconnect.only");

    snprintf(buf, MBUF_SIZE, "%s has disconnected.", Name(player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(player) && is_wizard(player)))
      key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);
    notify_checked(player, player, buf, key);
    free_mbuf(buf);

    if (mudconf.have_comsys)
      do_comdisconnect(player);

    raw_broadcast(MONITOR, "GAME: %s has disconnected.", Name(player));

    /* wait_que()'s argument array isn't const-correct; reason is only
       read as command-queue text here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    argv[0] = (char *)reason;
#pragma clang diagnostic pop
    c_connected(player);

    atr_temp = attribute_parent_get(player, A_ADISCONNECT, &aowner, &aflags);
    if (atr_temp && *atr_temp)
      wait_que(player, player, 0, NOTHING, 0, atr_temp, argv, 1, nullptr);
    free_lbuf(atr_temp);
    if (mudconf.master_room != NOTHING) {
      atr_temp = attribute_parent_get(mudconf.master_room, A_ADISCONNECT,
                                      &aowner, &aflags);
      if (atr_temp)
        wait_que(mudconf.master_room, player, 0, NOTHING, 0, atr_temp,
                 (char **)nullptr, 0, nullptr);
      free_lbuf(atr_temp);
      DOLIST(obj, obj_contents(mudconf.master_room)) {
        atr_temp = attribute_parent_get(obj, A_ADISCONNECT, &aowner, &aflags);
        if (atr_temp) {
          wait_que(obj, player, 0, NOTHING, 0, atr_temp, (char **)nullptr, 0,
                   nullptr);
        }
        free_lbuf(atr_temp);
      }
    }
    /*
     * do the zone of the player's location's possible * * *
     * adisconnect
     */
    if (mudconf.have_zones && ((zone = obj_zone(loc)) != NOTHING)) {
      switch (typeof_obj(zone)) {
      case TYPE_THING:
        atr_temp = attribute_parent_get(zone, A_ADISCONNECT, &aowner, &aflags);
        if (atr_temp) {
          wait_que(zone, player, 0, NOTHING, 0, atr_temp, (char **)nullptr, 0,
                   nullptr);
        }
        free_lbuf(atr_temp);
        break;
      case TYPE_ROOM:
        /*
         * check every object in the room for a * * *
         * connect action
         */
        DOLIST(obj, obj_contents(zone)) {
          atr_temp = attribute_parent_get(obj, A_ADISCONNECT, &aowner, &aflags);
          if (atr_temp) {
            wait_que(obj, player, 0, NOTHING, 0, atr_temp, (char **)nullptr, 0,
                     nullptr);
          }
          free_lbuf(atr_temp);
        }
        break;
      default:
        log_text(tprintf("Invalid zone #%ld for %s(#%ld) has bad type %d", zone,
                         Name(player), player, typeof_obj(zone)));
      }
    }
    if (d->flags & DS_AUTODARK) {
      s_flags(d->player, obj_flags(d->player) & ~DARK);
      d->flags &= ~DS_AUTODARK;
    }

  } else {
    buf = alloc_mbuf("descriptor_announce_disconnect.partial");
    snprintf(buf, MBUF_SIZE, "%s has partially disconnected.", Name(player));
    key = MSG_INV;
    if ((loc != NOTHING) && !(is_dark(player) && is_wizard(player)))
      key |= (MSG_NBR | MSG_NBR_EXITS | MSG_LOC | MSG_FWDLIST);
    notify_checked(player, player, buf, key);
    raw_broadcast(MONITOR, "GAME: %s has partially disconnected.",
                  Name(player));
    free_mbuf(buf);
  }

  mudstate.curr_enactor = temp;
}

int boot_off(DbRef player, const char *message) {
  Descriptor *d, *dnext;
  int count;

  count = 0;
  DESC_SAFEITER_PLAYER(player, d, dnext) {
    if (message && *message) {
      descriptor_queue_string(d, message);
      descriptor_queue_string(d, "\r\n");
    }
    descriptor_shutdown(d, R_BOOT);
    count++;
  }
  return count;
}

int boot_by_port(int port, int no_god, char *message) {
  Descriptor *d, *dnext;
  int count;

  count = 0;
  DESC_SAFEITER_ALL(d, dnext) {
    if ((d->descriptor == port) && (!no_god || !is_god(d->player))) {
      if (message && *message) {
        descriptor_queue_string(d, message);
        descriptor_queue_string(d, "\r\n");
      }
      descriptor_shutdown(d, R_BOOT);
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

void descriptor_reload(DbRef player) {
  Descriptor *d;
  char *buf;
  DbRef aowner;
  Flag aflags;

  DESC_ITER_PLAYER(player, d) {
    buf = attribute_parent_get(player, A_TIMEOUT, &aowner, &aflags);
    if (buf) {
      d->timeout = clamped_atoi(buf);
      if (d->timeout <= 0)
        d->timeout = mudconf.idle_timeout;
    }
    free_lbuf(buf);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * fetch_idle, fetch_connect: Return smallest idle time/largest connec time
 * * for a player (or -1 if not logged in)
 */

int fetch_idle(DbRef target) {
  Descriptor *d;
  int result, idletime;

  result = -1;
  DESC_ITER_PLAYER(target, d) {
    idletime = (int)(mudstate.now - d->last_time);
    if ((result == -1) || (idletime < result))
      result = idletime;
  }
  return result;
}

int fetch_connect(DbRef target) {
  Descriptor *d;
  int result, conntime;

  result = -1;
  DESC_ITER_PLAYER(target, d) {
    conntime = (int)(mudstate.now - d->connected_at);
    if (conntime > result)
      result = conntime;
  }
  return result;
}

static char *trimmed_name(DbRef player) {
  static char cbuff[18];

  if (strlen(Name(player)) <= 16)
    return Name(player);
  StringCopyTrunc(cbuff, Name(player), 16);
  cbuff[16] = '\0';
  return cbuff;
}

static void dump_users(Descriptor *e, char *match, int key) {
  Descriptor *d;
  int count, rcount, ucount;
  char *buf, *fp, *sp, flist[4], slist[4];
  DbRef room_it;

  while (match && *match && isspace(*match))
    match++;
  if (!match || !*match)
    match = nullptr;

  buf = alloc_lbuf("dump_users");
  if (key == CMD_SESSION) {
    descriptor_queue_string(e, "                               ");
    descriptor_queue_string(
        e, "     Characters Input----  Characters Output---\r\n");
  }
  descriptor_queue_string(e, "Player Name         On For  Idle ");
  if (key == CMD_SESSION) {
    descriptor_queue_string(
        e, "Port Pend  Lost     Total  Pend  Lost     Total\r\n");
  } else if ((e->flags & DS_CONNECTED) && is_wizard(e->player) &&
             (key == CMD_WHO)) {
    descriptor_queue_string(e, "     Room    Cmds Host\r\n");
  } else
    descriptor_queue_string(e, "\r\n");
  //	descriptor_queue_string(e,"------------------------------------------------------------------------------\r\n");
  count = 0;
  rcount = 0;
  ucount = 0;
  DESC_ITER_CONN(d) {
    if ((!mudconf.show_unfindable_who || !is_hidden(d->player)) ||
        (e->flags & DS_CONNECTED) & is_wizard(e->player)) {
      count++;
      if (match && !(string_prefix(Name(d->player), match)))
        continue;
      rcount++;
      if ((key == CMD_SESSION) &&
          !(is_wizard(e->player) && (e->flags & DS_CONNECTED)) &&
          (d->player != e->player))
        continue;

      /*
       * Get choice flags for wizards
       */

      fp = flist;
      sp = slist;
      if ((e->flags & DS_CONNECTED) && is_wizard(e->player)) {
        if (is_hidden(d->player)) {
          if (d->flags & DS_AUTODARK)
            *fp++ = 'd';
          else if (is_dark(d->player))
            *fp++ = 'D';
        }
        if (!is_findable(d->player)) {
          *fp++ = 'U';
        } else {
          room_it = where_room(d->player);
          if (is_good_obj(room_it)) {
            if (is_hideout(room_it))
              *fp++ = 'u';
          } else {
            *fp++ = 'u';
          }
        }

        if (is_suspect(d->player))
          *fp++ = '+';
        if (d->host_info & H_FORBIDDEN)
          *sp++ = 'F';
        if (d->host_info & H_SUSPECT)
          *sp++ = '+';
      }
      *fp = '\0';
      *sp = '\0';

      if ((e->flags & DS_CONNECTED) && is_wizard(e->player) &&
          (key == CMD_WHO)) {
        snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%-3s#%6ld %7d %-25s\r\n",
                 trimmed_name(d->player),
                 time_format_1(mudstate.now - d->connected_at),
                 time_format_2(mudstate.now - d->last_time), flist,
                 obj_location(d->player), d->command_count,
                 (d->username[0] != '\0')
                     ? tprintf("%s@%s", d->username, d->addr)
                     : d->addr);
      } else if (key == CMD_SESSION) {
        snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%5d%5d%6d%10d%6d%6d%10d\r\n",
                 trimmed_name(d->player),
                 time_format_1(mudstate.now - d->connected_at),
                 time_format_2((mudstate.now - d->last_time) > HIDDEN_IDLESECS
                                   ? (mudstate.now - d->last_time)
                                   : 0),
                 d->descriptor, d->input_size, d->input_lost, d->input_tot,
                 d->output_size, d->output_lost, d->output_tot);
      } else if (is_wizard(e->player)) {
        snprintf(buf, LBUF_SIZE, "%-16s%10s %5s%-3s\r\n",
                 trimmed_name(d->player),
                 time_format_1(mudstate.now - d->connected_at),
                 time_format_2((mudstate.now - d->last_time) > HIDDEN_IDLESECS
                                   ? (mudstate.now - d->last_time)
                                   : 0),
                 flist);
      } else {
        snprintf(buf, LBUF_SIZE, "%-16s%10s %5s\r\n", trimmed_name(d->player),
                 time_format_1(mudstate.now - d->connected_at),
                 time_format_2((mudstate.now - d->last_time) > HIDDEN_IDLESECS
                                   ? (mudstate.now - d->last_time)
                                   : 0));
      }
      descriptor_queue_string(e, buf);
    } else {
      ucount++;
    }
  }
  count = rcount; /* previous mode was .. disgusting. */
  /*
   * sometimes I like the ternary operator....
   */
  if (ucount)
    snprintf(buf, LBUF_SIZE,
             "%d Visible Player%slogged in, (%d %s hidden), %d record, %s "
             "maximum.\r\n",
             count, (count == 1) ? " " : "s ", ucount,
             (ucount == 1) ? "is" : "are", mudstate.record_players,
             (mudconf.max_players == -1) ? "no"
                                         : tprintf("%d", mudconf.max_players));
  else
    snprintf(buf, LBUF_SIZE, "%d Player%slogged in, %d record, %s maximum.\r\n",
             count, (count == 1) ? " " : "s ", mudstate.record_players,
             (mudconf.max_players == -1) ? "no"
                                         : tprintf("%d", mudconf.max_players));

  descriptor_queue_string(e, buf);

  free_lbuf(buf);
}

NameTable logout_cmdtable[] = {
    {"DOING", 5, CA_PUBLIC, CMD_DOING},
    {"OUTPUTPREFIX", 12, CA_PUBLIC, CMD_PREFIX | CMD_NOxFIX},
    {"OUTPUTSUFFIX", 12, CA_PUBLIC, CMD_SUFFIX | CMD_NOxFIX},
    {"QUIT", 4, CA_PUBLIC, CMD_QUIT},
    {"SESSION", 7, CA_PUBLIC, CMD_SESSION},
    {"WHO", 3, CA_PUBLIC, CMD_WHO},
    {nullptr, 0, 0, 0}};

void init_logout_cmdtab(void) {
  NameTable *cp;

  /*
   * Make the htab bigger than the number of entries so that we find
   * things on the first check.  Remember that the admin can add
   * aliases.
   */

  hash_table_initialize(&mudstate.logout_cmd_htab, 3 * HASH_FACTOR);
  for (cp = logout_cmdtable; cp->flag; cp++)
    hash_table_add(cp->name, (int *)cp, &mudstate.logout_cmd_htab);
}

int descriptor_command(Descriptor *d, char *command) {
  char *arg;
  const char *cmdsave;
  NameTable *cp;

  cmdsave = mudstate.debug_cmd;
  mudstate.debug_cmd = "< descriptor_command >";

  /* The IDLE command is used to keep players behind badly configured NATs
     alive. This does not increment command count or idle time and is a
     good alternative to a lot of the current anti-disconnectors out there.
  */
  if (!strcasecmp(command, "IDLE") && d->flags & DS_CONNECTED) {
    mudstate.curr_player = d->player;
    mudstate.curr_enactor = d->player;
    mudstate.debug_cmd = "idle";
    mudstate.debug_cmd = cmdsave;
    return 1;
  }

  /*
   * Split off the command from the arguments
   */

  arg = command;
  while (*arg && !isspace(*arg))
    arg++;
  if (*arg)
    *arg++ = '\0';

  d->last_time = mudstate.now;

  cp = (NameTable *)hash_table_find(command, &mudstate.logout_cmd_htab);

  if (*arg)
    *--arg = ' ';
  if (cp == nullptr) {
    d->command_count++;
    if (d->output_prefix) {
      descriptor_queue_string(d, d->output_prefix);
    }
    mudstate.curr_player = d->player;
    mudstate.curr_enactor = d->player;
    mudstate.curr_descriptor = d;
    process_command(d->player, d->player, 1, command, (char **)nullptr, 0);
    mudstate.curr_descriptor = nullptr;
    if (d->output_suffix) {
      descriptor_queue_string(d, d->output_suffix);
    }
  } else {
    if (d->output_prefix) {
      descriptor_queue_string(d, d->output_prefix);
    }
    switch (cp->flag & CMD_MASK) {
    case CMD_QUIT:
      descriptor_shutdown(d, R_QUIT);
      return 0;
    case CMD_WHO:
      if (d->player || mudconf.allow_unloggedwho) {
        dump_users(d, arg, CMD_WHO);
      } else {
        descriptor_queue_string(
            d, "This MUX does not allow WHO at the login screen.\r\n");
        descriptor_queue_string(
            d, "Please login or create a character first.\r\n");
      }
      break;
    case CMD_DOING:
      if (d->player || mudconf.allow_unloggedwho) {
        dump_users(d, arg, CMD_DOING);
      } else {
        descriptor_queue_string(
            d, "This MUX does not allow DOING at the login screen.\r\n");
        descriptor_queue_string(
            d, "Please login or create a character first.\r\n");
      }
      break;
    case CMD_SESSION:
      if (d->player || mudconf.allow_unloggedwho) {
        dump_users(d, arg, CMD_SESSION);
      } else {
        descriptor_queue_string(
            d, "This MUX does not allow SESSION at the login screen.\r\n");
        descriptor_queue_string(
            d, "Please login or create a character first.\r\n");
      }
      break;
    case CMD_PREFIX:
      set_userstring(&d->output_prefix, arg);
      break;
    case CMD_SUFFIX:
      set_userstring(&d->output_suffix, arg);
      break;
    default:
      log_error(LOG_BUGS, "BUG", "PARSE",
                "Prefix command with no handler: '%s'", command);
    }
    if (d->output_suffix) {
      descriptor_queue_string(d, d->output_suffix);
    }
  }
  mudstate.debug_cmd = cmdsave;
  return 1;
}

/*
 * ---------------------------------------------------------------------------
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

static void list_sites(DbRef player, SiteData *site_list,
                       const char *header_txt, int stat_type) {
  char *buff, *buff1;
  const char *str;
  SiteData *this;

  buff = alloc_mbuf("list_sites.buff");
  buff1 = alloc_sbuf("list_sites.addr");
  snprintf(buff, MBUF_SIZE, "----- %s -----", header_txt);
  notify(player, buff);
  notify(player, "Address              Mask                 Status");
  for (this = site_list; this; this = this->next) {
    str = stat_string(stat_type, this->flag);
    StringCopy(buff1, inet_ntoa(this->mask));
    snprintf(buff, MBUF_SIZE, "%-20s %-20s %s", inet_ntoa(this->address), buff1,
             str);
    notify(player, buff);
  }
  free_mbuf(buff);
  free_sbuf(buff1);
}

/*
 * ---------------------------------------------------------------------------
 * * list_siteinfo: List information about specially-marked sites.
 */

void list_siteinfo(DbRef player) {
  list_sites(player, mudstate.access_list, "Site Access", S_ACCESS);
  list_sites(player, mudstate.suspect_list, "Suspected Sites", S_SUSPECT);
}

/*
 * ---------------------------------------------------------------------------
 * * make_ulist: Make a list of connected user numbers for the LWHO function.
 */

void make_ulist(DbRef player, char *buff, char **bufc) {
  Descriptor *d;
  char *cp;

  cp = *bufc;
  DESC_ITER_CONN(d) {
    if (!is_wizard(player) && is_hidden(d->player))
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

DbRef find_connected_name(DbRef player, char *name) {
  Descriptor *d;
  DbRef found;

  found = NOTHING;
  DESC_ITER_CONN(d) {
    if (is_good_obj(player) && !is_wizard(player) && is_hidden(d->player))
      continue;
    if (!string_prefix(Name(d->player), name))
      continue;
    if ((found != NOTHING) && (found != d->player))
      return NOTHING;
    found = d->player;
  }
  return found;
}

Descriptor *descriptor_find_by_fd(int fd) {
  Descriptor *d;

  DESC_ITER_CONN(d) {
    if (d->descriptor == fd)
      return d;
  }
  return nullptr;
}

void descriptor_run_command(Descriptor *d, char *command) {
  if (!is_wizard(d->player)) {
    if (d->quota <= 0) {
      descriptor_queue_string(d, "quota exceed, dropping command.\n");
      dprintk("aborting execution of %s for #%ld.", command, d->player);
      return;
    }
    d->quota--;
  }
  descriptor_command(d, command);
}
