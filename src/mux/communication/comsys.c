/* comsys.c - Player channel creation, membership, and message delivery. */

#include <ctype.h>
#include <sys/types.h>

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"

#include "mux/commands/functions.h"
#include "mux/communication/comsys.h"
#include "mux/network/mux_event_alloc.h"

int num_channels;
int max_channels;

/* Static functions */
static void do_show_com(void *);
static void do_comlast(DbRef, struct channel *);
static void do_comsend(struct channel *, char *);
static void do_comprintf(struct channel *, char *, ...);
static void do_leavechannel(DbRef, struct channel *);
static void do_comwho(DbRef, struct channel *);
static void do_setnewtitle(DbRef, struct channel *, char *);
static int do_test_access(DbRef, long, struct channel *);
static char *get_channel_from_alias(DbRef, char *);
static void do_processcom(DbRef, char *, char *);
static void do_delcomchannel(DbRef, char *);
static void do_listchannels(DbRef);
static void do_comdisconnectraw_notify(DbRef, char *);
static void do_comconnectraw_notify(DbRef, char *);
static void do_comconnectchannel(DbRef, char *, char *, int);
static void do_comdisconnectchannel(DbRef, char *);
static void do_chclose(DbRef, char *);
static void do_chloud(DbRef, char *);
static void do_chsquelch(DbRef, char *);
static void do_chtransparent(DbRef, char *);
static void do_chopaque(DbRef, char *);
static void do_chanobj(DbRef, char *, char *);

/*
 * This is the hash table for channel names
 */

void init_chantab(void) {
  hash_table_initialize(&mudstate.channel_htab, 30 * HASH_FACTOR);
}

void send_channel(char *chan, const char *format, ...) {
  struct channel *ch;
  char buf[LBUF_SIZE];
  char data[LBUF_SIZE];
  char *bp = buf;
  char *newline;
  va_list ap;

  if (!(ch = select_channel(chan)))
    return;
  va_start(ap, format);
  vsnprintf(data, LBUF_SIZE, format, ap);
  va_end(ap);

  safe_chr('[', buf, &bp);
  safe_str(chan, buf, &bp);
  safe_str("] ", buf, &bp);
  safe_str(data, buf, &bp);
  *bp = '\0';
  while ((newline = strchr(buf, '\n')))
    *newline = ' ';
  do_comsend(ch, buf);
}

static char *get_channel_from_alias(DbRef player, char *alias) {
  struct commac *c;
  int first, last, current = 0;
  int dir;

  c = get_commac(player);

  first = 0;
  last = c->numchannels - 1;
  dir = 1;

  while (dir && (first <= last)) {
    current = (first + last) / 2;
    dir = strcasecmp(alias, c->alias + 6 * current);
    if (dir < 0)
      last = current - 1;
    else
      first = current + 1;
  }

  if (!dir)
    return c->channels[current];
  else
    return "";
}

static DbRef cheat_player;

static void do_show_com(void *data) {
  chmsg *d = data;
  struct tm *t;
  int day;
  char buf[LBUF_SIZE];

  t = localtime(&mudstate.now);
  day = t->tm_mday;
  t = localtime(&d->time);
  if (day == t->tm_mday) {
    snprintf(buf, sizeof(buf), "[%02d:%02d] %s", t->tm_hour, t->tm_min, d->msg);
  } else
    snprintf(buf, sizeof(buf), "[%02d.%02d / %02d:%02d] %s", t->tm_mon + 1,
             t->tm_mday, t->tm_hour, t->tm_min, d->msg);
  notify(cheat_player, buf);
}

static void do_comlast(DbRef player, struct channel *ch) {
  if (!fifo_length(&ch->last_messages)) {
    notify_printf(player, "There haven't been any messages on %s.", ch->name);
    return;
  }
  cheat_player = player;
  fifo_traverse_reverse(&ch->last_messages, do_show_com);
}

static void do_processcom(DbRef player, char *arg1, char *arg2) {
  struct channel *ch;
  struct comuser *user;

  if ((strlen(arg1) + strlen(arg2)) > LBUF_SIZE / 2) {
    arg2[LBUF_SIZE / 2 - strlen(arg1)] = '\0';
  }
  if (!*arg2) {
    raw_notify(player, "No message.");
    return;
  }

  if (!is_wizard(player) && is_in_character_location(player)) {
    raw_notify(player, "Permission denied.");
    return;
  }

  if (!(ch = select_channel(arg1))) {
    notify_printf(player, "Unknown channel %s.", arg1);
    return;
  }
  if (!(user = select_user(ch, player))) {
    raw_notify(player, "You are not listed as on that channel.  Delete this "
                       "alias and re-add.");
    return;
  }
  if (!strcasecmp(arg2, "on")) {
    do_joinchannel(player, ch);
  } else if (!strcasecmp(arg2, "off")) {
    do_leavechannel(player, ch);
  } else if (!user->on && !is_wizard(player) && !mudconf.allow_chanlurking) {
    notify_printf(player, "You must be on %s to do that.", arg1);
    return;
  } else if (!strcasecmp(arg2, "who")) {
    do_comwho(player, ch);
  } else if (!strcasecmp(arg2, "last")) {
    do_comlast(player, ch);
  } else if (!user->on) {
    notify_printf(player, "You must be on %s to do that.", arg1);
    return;
  } else if (!do_test_access(player, CHANNEL_TRANSMIT, ch)) {
    raw_notify(player, "That channel type cannot be transmitted on.");
    return;
  } else {
    if ((*arg2) == ':')
      do_comprintf(ch, "[%s] %s %s", arg1, Name(player), arg2 + 1);
    else if ((*arg2) == ';')
      do_comprintf(ch, "[%s] %s%s", arg1, Name(player), arg2 + 1);
    else if (strlen(user->title))
      do_comprintf(ch, "[%s] %s: <%s> %s", arg1, Name(player), user->title,
                   arg2);
    else
      do_comprintf(ch, "[%s] %s: %s", arg1, Name(player), arg2);
  }
}

static void do_comsend(struct channel *ch, char *mess) {
  struct comuser *user;
  chmsg *c;

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on && do_test_access(user->who, CHANNEL_RECIEVE, ch) &&
        (is_wizard(user->who) || !is_in_character_location(user->who))) {
      if (typeof_obj(user->who) == TYPE_PLAYER && is_connected(user->who))
        raw_notify(user->who, mess);
      else
        notify(user->who, mess);
    }
  }
  /* Also, add it to the history of channel */
  if (fifo_length(&ch->last_messages) >= CHANNEL_HISTORY_LEN) {
    c = fifo_pop(&ch->last_messages);
    free((void *)c->msg);
  } else
    Create(c, chmsg, 1);
  c->msg = strdup(mess);
  c->time = mudstate.now;
  fifo_push(&ch->last_messages, c);
}

static void do_comprintf(struct channel *ch, char *messfmt, ...)
    __attribute__((format(printf, 2, 3)));

static void do_comprintf(struct channel *ch, char *messfmt, ...) {
  struct comuser *user;
  chmsg *c;
  va_list ap;
  char buffer[LBUF_SIZE];
  memset(buffer, 0, LBUF_SIZE);
  va_start(ap, messfmt);
  vsnprintf(buffer, LBUF_SIZE - 1, messfmt, ap);
  va_end(ap);

  ch->num_messages++;
  for (user = ch->on_users; user; user = user->on_next) {
    if (user->on && do_test_access(user->who, CHANNEL_RECIEVE, ch) &&
        (is_wizard(user->who) || !is_in_character_location(user->who))) {
      if (typeof_obj(user->who) == TYPE_PLAYER && is_connected(user->who))
        raw_notify(user->who, buffer);
      else
        notify(user->who, buffer);
    }
  }
  /* Also, add it to the history of channel */
  if (fifo_length(&ch->last_messages) >= CHANNEL_HISTORY_LEN) {
    c = fifo_pop(&ch->last_messages);
    free((void *)c->msg);
  } else
    Create(c, chmsg, 1);
  c->msg = strdup(buffer);
  c->time = mudstate.now;
  fifo_push(&ch->last_messages, c);
}

void do_joinchannel(DbRef player, struct channel *ch) {
  struct comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user) {
    ch->num_users++;
    if (ch->num_users >= ch->max_users) {
      ch->max_users += 10;
      ch->users = realloc(ch->users, sizeof(struct comuser *) * ch->max_users);
      memset(ch->users + (ch->num_users - 1), 0,
             sizeof(struct comuser *) * (ch->max_users - ch->num_users));
    }
    user = (struct comuser *)malloc(sizeof(struct comuser));

    for (i = ch->num_users - 1; i > 0 && ch->users[i - 1]->who > player; i--)
      ch->users[i] = ch->users[i - 1];
    ch->users[i] = user;

    user->who = player;
    user->on = 1;
    user->title = strdup("");

    if (is_undead(player)) {
      user->on_next = ch->on_users;
      ch->on_users = user;
    }
  } else if (!user->on) {
    user->on = 1;
  } else {
    notify_printf(player, "You are already on channel %s.", ch->name);
    return;
  }
#if 0
	/* Trigger AENTER of any channel objects on the channel */
	for(i = ch->num_users - 1; i > 0; i--) {
		if(!ch->users[i]) break;
		if(typeof_obj(ch->users[i]->who) == TYPE_THING)
			did_it(player, ch->users[i]->who, 0, nullptr, 0, nullptr, A_AENTER,
				   (char **) nullptr, 0);
	}
#endif
  notify_printf(player, "You have joined channel %s.", ch->name);

  if (!is_dark(player)) {
    do_comprintf(ch, "[%s] %s has joined this channel.", ch->name,
                 Name(player));
  }
}

static void do_leavechannel(DbRef player, struct channel *ch) {
  struct comuser *user;
  int i;

  user = select_user(ch, player);

  if (!user)
    return;

  /* Trigger ALEAVE of any channel objects on the channel */
  for (i = ch->num_users - 1; i > 0; i--) {
    if (typeof_obj(ch->users[i]->who) == TYPE_THING)
      did_it(player, ch->users[i]->who, 0, nullptr, 0, nullptr, A_ALEAVE,
             (char **)nullptr, 0);
  }

  notify_printf(player, "You have left channel %s.", ch->name);

  if ((user->on) && (!is_dark(player))) {
    char *c = Name(player);

    if (c && *c) {
      do_comprintf(ch, "[%s] %s has left this channel.", ch->name, c);
    }
  }
  user->on = 0;
}

static void do_comwho(DbRef player, struct channel *ch) {
  struct comuser *user;
  char *buff;

  raw_notify(player, "-- Players --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(user->who) == TYPE_PLAYER && user->on &&
        is_connected(user->who) &&
        (!is_hidden(user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) && !is_dark(user->who)) ||
         is_wizard(player)) &&
        (!is_in_character_location(user->who) || is_wizard(user->who))) {

      int i = fetch_idle(user->who);

      buff = unparse_object(player, user->who, 0);
      if (i > 30) {
        char *c = get_uptime_to_string(i);

        notify_printf(player, "%s [idle %s]", buff, c);
        free_sbuf(c);
      } else
        notify_printf(player, "%s", buff);
      free_lbuf(buff);
    }
  }

  raw_notify(player, "-- Objects --");
  for (user = ch->on_users; user; user = user->on_next) {
    if (typeof_obj(user->who) != TYPE_PLAYER && user->on &&
        !(is_going(user->who) && is_god(obj_owner(user->who)))) {
      buff = unparse_object(player, user->who, 0);
      notify_printf(player, "%s", buff);
      free_lbuf(buff);
    }
  }
  notify_printf(player, "-- %s --", ch->name);
}

struct channel *select_channel(char *channel) {
  return (struct channel *)hash_table_find(channel, &mudstate.channel_htab);
}

struct comuser *select_user(struct channel *ch, DbRef player) {
  int last, current;
  int dir = 1, first = 0;

  if (!ch)
    return nullptr;

  last = ch->num_users - 1;
  current = (first + last) / 2;

  while (dir && (first <= last)) {
    current = (first + last) / 2;
    if (ch->users[current] == nullptr) {
      last--;
      continue;
    }
    if (ch->users[current]->who == player)
      dir = 0;
    else if (ch->users[current]->who < player) {
      dir = 1;
      first = current + 1;
    } else {
      dir = -1;
      last = current - 1;
    }
  }

  if (!dir)
    return ch->users[current];
  else
    return nullptr;
}

void do_addcom(DbRef player, DbRef cause, int key, char *arg1, char *arg2) {
  char channel[200];
  char title[100];
  struct channel *ch;
  char *s;
  int where;
  struct commac *c;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!*arg1) {
    raw_notify(player, "You need to specify an alias.");
    return;
  }
  if (!*arg2) {
    raw_notify(player, "You need to specify a channel.");
    return;
  }

  s = strchr(arg2, ',');

  if (s) {
    /* channelname,title */
    if (s >= arg2 + 200) {
      raw_notify(player, "Channel name too long.");
      return;
    }
    strncpy(channel, arg2, s - arg2);
    channel[s - arg2] = '\0';
    strncpy(title, s + 1, 100);
    title[99] = '\0';
  } else {
    /* just channelname */
    if (strlen(arg2) >= 200) {
      raw_notify(player, "Channel name too long.");
      return;
    }
    strlcpy(channel, arg2, sizeof(channel));
    title[0] = '\0';
  }

  if (strchr(channel, ' ')) {
    raw_notify(player, "Channel name cannot contain spaces.");
    return;
  }

  if (!(ch = select_channel(channel))) {
    notify_printf(player, "Channel %s does not exist yet.", channel);
    return;
  }
  if (!do_test_access(player, CHANNEL_JOIN, ch)) {
    raw_notify(player, "Sorry, this channel type does not allow you to join.");
    return;
  }
  if (select_user(ch, player)) {
    raw_notify(player, "Warning: you are already listed on that channel.");
  }
  c = get_commac(player);
  for (where = 0;
       where < c->numchannels && (strcasecmp(arg1, c->alias + where * 6) > 0);
       where++)
    ;
  if (where < c->numchannels && !strcasecmp(arg1, c->alias + where * 6)) {
    notify_printf(player, "That alias is already in use for channel %s.",
                  c->channels[where]);
    return;
  }
  if (c->numchannels >= c->maxchannels) {
    c->maxchannels += 10;
    c->alias = realloc(c->alias, sizeof(char) * 6 * c->maxchannels);
    c->channels = realloc(c->channels, sizeof(char *) * c->maxchannels);
  }
  if (where < c->numchannels) {
    memmove(c->alias + 6 * (where + 1), c->alias + 6 * where,
            6 * (c->numchannels - where));
    memmove(c->channels + where + 1, c->channels + where,
            sizeof(c->channels) * (c->numchannels - where));
  }

  c->numchannels++;

  strncpy(c->alias + 6 * where, arg1, 5);
  c->alias[where * 6 + 5] = '\0';
  c->channels[where] = strdup(ch->name);

  do_joinchannel(player, ch);
  do_setnewtitle(player, ch, title);

  if (title[0])
    notify_printf(player, "Channel %s added with alias %s and title %s.",
                  ch->name, arg1, title);
  else
    notify_printf(player, "Channel %s added with alias %s.", ch->name, arg1);
}

static void do_setnewtitle(DbRef player, struct channel *ch, char *title) {
  struct comuser *user;
  char *new;

  user = select_user(ch, player);

  /* Make sure there can be no embedded newlines from %r */

  if (!ch || !user)
    return;

  new = replace_string("\r\n", "", title);
  if (user->title)
    free(user->title);
  user->title = strdup(new); /* strdup so we can free() safely */
  free_lbuf(new);
}

void do_delcom(DbRef player, DbRef cause, int key, char *arg1) {
  int i;
  struct commac *c;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!arg1) {
    raw_notify(player, "Need an alias to delete.");
    return;
  }
  c = get_commac(player);

  for (i = 0; i < c->numchannels; i++) {
    if (!strcasecmp(arg1, c->alias + i * 6)) {
      do_delcomchannel(player, c->channels[i]);
      notify_printf(player, "Channel %s deleted.", c->channels[i]);
      free(c->channels[i]);

      c->numchannels--;
      if (i < c->numchannels) {
        memmove(c->alias + 6 * i, c->alias + 6 * (i + 1),
                6 * (c->numchannels - i));
        memmove(c->channels + i, c->channels + i + 1,
                sizeof(c->channels) * (c->numchannels - i));
      }
      return;
    }
  }
  raw_notify(player, "Unable to find that alias.");
}

static void do_delcomchannel(DbRef player, char *channel) {
  struct channel *ch;
  struct comuser *user;
  int i;

  if (!(ch = select_channel(channel))) {
    notify_printf(player, "Unknown channel %s.", channel);
  } else {

    /* Trigger ALEAVE of any channel objects on the channel */
    for (i = ch->num_users - 1; i > 0; i--) {
      if (typeof_obj(ch->users[i]->who) == TYPE_THING)
        did_it(player, ch->users[i]->who, 0, nullptr, 0, nullptr, A_ALEAVE,
               (char **)nullptr, 0);
    }

    for (i = 0; i < ch->num_users; i++) {
      user = ch->users[i];
      if (user->who == player) {
        do_comdisconnectchannel(player, channel);
        if (user->on && !is_dark(player)) {
          char *c = Name(player);

          if (c && *c)
            do_comprintf(ch, "[%s] %s has left this channel.", channel, c);
        }
        notify_printf(player, "You have left channel %s.", channel);

        if (user->title)
          free(user->title);
        free(user);
        ch->num_users--;
        if (i < ch->num_users)
          memmove(ch->users + i, ch->users + i + 1,
                  sizeof(ch->users) * (ch->num_users - i));
      }
    }
  }
}

void do_createchannel(DbRef player, DbRef cause, int key, char *channel) {
  struct channel *newchannel;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (select_channel(channel)) {
    notify_printf(player, "Channel %s already exists.", channel);
    return;
  }
  if (!*channel) {
    raw_notify(player, "You must specify a channel to create.");
    return;
  }
  if (!(is_comm_all(player))) {
    raw_notify(player, "You do not have permission to do that.");
    return;
  }
  newchannel = (struct channel *)malloc(sizeof(struct channel));

  strncpy(newchannel->name, channel, CHAN_NAME_LEN - 1);
  newchannel->name[CHAN_NAME_LEN - 1] = '\0';
  newchannel->last_messages = nullptr;
  newchannel->type = 127;
  newchannel->temp1 = 0;
  newchannel->temp2 = 0;
  newchannel->charge = 0;
  newchannel->charge_who = player;
  newchannel->amount_col = 0;
  newchannel->num_users = 0;
  newchannel->max_users = 0;
  newchannel->users = nullptr;
  newchannel->on_users = nullptr;
  newchannel->chan_obj = NOTHING;
  newchannel->num_messages = 0;

  num_channels++;

  hash_table_add(newchannel->name, (int *)newchannel, &mudstate.channel_htab);

  notify_printf(player, "Channel %s created.", channel);
}

void do_destroychannel(DbRef player, DbRef cause, int key, char *channel) {
  struct channel *ch;
  int j;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  ch = (struct channel *)hash_table_find(channel, &mudstate.channel_htab);

  if (!ch) {
    notify_printf(player, "Could not find channel %s.", channel);
    return;
  } else if (!(is_comm_all(player)) && (player != ch->charge_who)) {
    raw_notify(player, "You do not have permission to do that. ");
    return;
  }
  num_channels--;
  hash_table_delete(channel, &mudstate.channel_htab);

  for (j = 0; j < ch->num_users; j++) {
    free(ch->users[j]);
  }
  free(ch->users);
  free(ch);
  notify_printf(player, "Channel %s destroyed.", channel);
}

static void do_listchannels(DbRef player) {
  struct channel *ch;
  int perm;

  if (!(perm = is_comm_all(player))) {
    raw_notify(
        player,
        "Warning: Only public channels and your channels will be shown.");
  }
  raw_notify(player,
             "** Channel             --Flags--  Obj   Owner Users   Messages");

  for (ch = (struct channel *)hash_table_first_entry(&mudstate.channel_htab);
       ch; ch = (struct channel *)hash_table_next_entry(&mudstate.channel_htab))
    if (perm || (ch->type & CHANNEL_PUBLIC) || ch->charge_who == player) {

      notify_printf(
          player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %5d %6d %10d",
          (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
          (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J' : '-',
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X' : '-',
          (ch->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT)) ? 'x' : '-',
          (ch->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r' : '-',
          (ch->chan_obj != NOTHING) ? ch->chan_obj : -1, ch->charge_who,
          ch->num_users, ch->num_messages);
    }
  raw_notify(player, "-- End of list of Channels --");
}

void do_comtitle(DbRef player, DbRef cause, int key, char *arg1, char *arg2) {
  struct channel *ch;
  char channel[100];

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!*arg1) {
    raw_notify(player, "Need an alias to do comtitle.");
    return;
  }
  strncpy(channel, get_channel_from_alias(player, arg1), 100);
  channel[99] = '\0';

  if (!*channel) {
    raw_notify(player, "Unknown alias");
    return;
  }
  if ((ch = select_channel(channel)) && select_user(ch, player)) {
    notify_printf(player, "Title set to '%s' on channel %s.", arg2, channel);
    do_setnewtitle(player, ch, arg2);
  }
  if (!ch) {
    raw_notify(player, "Invalid comsys alias, please delete.");
    return;
  }
}

void do_comlist(DbRef player, DbRef cause, int key) {
  struct comuser *user;
  struct commac *c;
  int i;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  c = get_commac(player);

  raw_notify(player, "Alias     Channel             Title                      "
                     "             Status");

  for (i = 0; i < c->numchannels; i++) {
    if ((user = select_user(select_channel(c->channels[i]), player))) {
      notify_printf(player, "%-9.9s %-19.19s %-39.39s %s", c->alias + i * 6,
                    c->channels[i], user->title, (user->on ? "on" : "off"));
    } else {
      notify_printf(player, "Bad Comsys Alias: %s for Channel: %s",
                    c->alias + i * 6, c->channels[i]);
    }
  }
  raw_notify(player, "-- End of comlist --");
}

void do_channelnuke(DbRef player) {
  struct channel *ch;
  int j;

  for (ch = (struct channel *)hash_table_first_entry(&mudstate.channel_htab);
       ch;
       ch = (struct channel *)hash_table_next_entry(&mudstate.channel_htab)) {
    if (ch->charge_who == player) {
      num_channels--;
      hash_table_delete(ch->name, &mudstate.channel_htab);

      for (j = 0; j < ch->num_users; j++)
        free(ch->users[j]);
      free(ch->users);
      free(ch);
    }
  }
}

void do_clearcom(DbRef player, DbRef cause, int key) {
  int i;
  struct commac *c;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  c = get_commac(player);

  for (i = (c->numchannels) - 1; i > -1; --i) {
    do_delcom(player, player, 0, c->alias + i * 6);
  }
}

void do_allcom(DbRef player, DbRef cause, int key, char *arg1) {
  int i;
  struct commac *c;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  c = get_commac(player);

  if ((strcasecmp(arg1, "who") != 0) && (strcasecmp(arg1, "on") != 0) &&
      (strcasecmp(arg1, "off") != 0)) {
    raw_notify(player, "Only options available are: on, off and who.");
    return;
  }
  for (i = 0; i < c->numchannels; i++) {
    do_processcom(player, c->channels[i], arg1);
    if (strcasecmp(arg1, "who") == 0)
      raw_notify(player, "");
  }
}

void do_channelwho(DbRef player, DbRef cause, int key, char *arg1) {
  struct channel *ch;
  struct comuser *user;
  char channel[100];
  int flag = 0;
  char *cp;
  int i;
  char ansibuffer[LBUF_SIZE];

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  cp = strchr(arg1, '/');
  if (!cp) {
    strncpy(channel, arg1, 100);
    channel[99] = '\0';
  } else {
    /* channelname/all */
    if (cp - arg1 >= 100) {
      raw_notify(player, "Channel name too long.");
      return;
    }
    strncpy(channel, arg1, cp - arg1);
    channel[cp - arg1] = '\0';
    if (*++cp == 'a')
      flag = 1;
  }

  if (!(ch = select_channel(channel))) {
    notify_printf(player, "Unknown channel \"%s\".", channel);
    return;
  }
  if (!((is_comm_all(player)) || (player == ch->charge_who))) {
    raw_notify(player, "You do not have permission to do that.");
    return;
  }
  notify_printf(player, "-- %s --", ch->name);
  notify_printf(player, "%-29.29s %-6.6s %-6.6s", "Name", "Status", "Player");
  for (i = 0; i < ch->num_users; i++) {
    user = ch->users[i];
    if ((flag || is_undead(user->who)) &&
        (!is_hidden(user->who) ||
         ((ch->type & CHANNEL_TRANSPARENT) && !is_dark(user->who)) ||
         is_wizard(player))) {
      cp = unparse_object(player, user->who, 0);
      strip_ansi_r(ansibuffer, cp, LBUF_SIZE);
      notify_printf(player, "%-29.29s %-6.6s %-6.6s", ansibuffer,
                    ((user->on) ? "on " : "off"),
                    (typeof_obj(user->who) == TYPE_PLAYER) ? "yes" : "no ");
      free_lbuf(cp);
    }
  }
  notify_printf(player, "-- %s --", ch->name);
}

static void do_comdisconnectraw_notify(DbRef player, char *chan) {
  struct channel *ch;
  struct comuser *cu;

  if (!(ch = select_channel(chan)))
    return;
  if (!(cu = select_user(ch, player)))
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) && (!is_dark(player))) {
    do_comprintf(ch, "[%s] %s has disconnected.", ch->name, Name(player));
  }
}

static void do_comconnectraw_notify(DbRef player, char *chan) {
  struct channel *ch;
  struct comuser *cu;

  if (!(ch = select_channel(chan)))
    return;
  if (!(cu = select_user(ch, player)))
    return;

  if ((ch->type & CHANNEL_LOUD) && (cu->on) && (!is_dark(player))) {
    do_comprintf(ch, "[%s] %s has connected.", ch->name, Name(player));
  }
}

static void do_comconnectchannel(DbRef player, char *channel, char *alias,
                                 int i) {
  struct channel *ch;
  struct comuser *user;

  if ((ch = select_channel(channel))) {
    for (user = ch->on_users; user && user->who != player; user = user->on_next)
      ;

    if (!user) {
      if ((user = select_user(ch, player))) {
        user->on_next = ch->on_users;
        ch->on_users = user;
      } else
        notify_printf(player, "Bad Comsys Alias: %s for Channel: %s",
                      alias + i * 6, channel);
    }
  } else
    notify_printf(player, "Bad Comsys Alias: %s for Channel: %s", alias + i * 6,
                  channel);
}

void do_comdisconnect(DbRef player) {
  int i;
  struct commac *c;

  c = get_commac(player);

  for (i = 0; i < c->numchannels; i++) {
    do_comdisconnectchannel(player, c->channels[i]);
    do_comdisconnectraw_notify(player, c->channels[i]);
  }
  send_channel("MUXConnections", "* %s has disconnected *", Name(player));
}

void do_comconnect(DbRef player, Descriptor *d) {
  struct commac *c;
  int i;
  char *lsite;

  c = get_commac(player);

  for (i = 0; i < c->numchannels; i++) {
    do_comconnectchannel(player, c->channels[i], c->alias, i);
    do_comconnectraw_notify(player, c->channels[i]);
  }
  lsite = d->addr;
  if (lsite && *lsite)
    send_channel("MUXConnections", "* %s has connected from %s *", Name(player),
                 lsite);
  else
    send_channel("MUXConnections", "* %s has connected from somewhere *",
                 Name(player));
}

static void do_comdisconnectchannel(DbRef player, char *channel) {
  struct comuser *user, *prevuser = nullptr;
  struct channel *ch;

  if (!(ch = select_channel(channel)))
    return;
  for (user = ch->on_users; user;) {
    if (user->who == player) {
      if (prevuser)
        prevuser->on_next = user->on_next;
      else
        ch->on_users = user->on_next;
      return;
    } else {
      prevuser = user;
      user = user->on_next;
    }
  }
}

void do_editchannel(DbRef player, DbRef cause, int flag, char *arg1,
                    char *arg2) {
  char *s;
  struct channel *ch;
  int add_remove = 1;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(arg1))) {
    notify_printf(player, "Unknown channel %s.", arg1);
    return;
  }
  if (!((is_comm_all(player)) || (player == ch->charge_who))) {
    raw_notify(player, "Permission denied.");
    return;
  }
  s = arg2;
  if (*s == '!') {
    add_remove = 0;
    s++;
  }
  switch (flag) {
  case 0:
    if (lookup_player(player, arg2, 1) != NOTHING) {
      ch->charge_who = lookup_player(player, arg2, 1);
      raw_notify(player, "Set.");
      return;
    } else {
      raw_notify(player, "Invalid player.");
      return;
    }
  case 3:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_JOIN));
      raw_notify(player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_RECIEVE));
      raw_notify(player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_PL_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_PL_MULT * CHANNEL_TRANSMIT));
      raw_notify(player,
                 (add_remove) ? "@cpflags: Set." : "@cpflags: Cleared.");
      return;
    }
    raw_notify(player, "@cpflags: Unknown Flag.");
    break;
  case 4:
    if (strcasecmp(s, "join") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_JOIN))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_JOIN));
      raw_notify(player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "receive") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_RECIEVE));
      raw_notify(player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    if (strcasecmp(s, "transmit") == 0) {
      add_remove ? (ch->type |= (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT))
                 : (ch->type &= ~(CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT));
      raw_notify(player,
                 (add_remove) ? "@coflags: Set." : "@coflags: Cleared.");
      return;
    }
    raw_notify(player, "@coflags: Unknown Flag.");
    break;
  }
  return;
}

static int do_test_access(DbRef player, long access, struct channel *chan) {
  long flag_value = access;

  if (is_comm_all(player))
    return (1);

  /*
   * Channel objects allow custom locks for channels.  The normal
   * lock is used to see if they can join that channel.  The
   * enterlock is checked to see if they can receive messages on
   * it. The Uselock is checked to see if they can transmit on
   * it. Note: These checks do not supercede the normal channel
   * flags. If a channel is set JOIN for players, ALL players can
   * join the channel, whether or not they pass the lock.  Same for
   * all channel object locks.
   */

  if ((flag_value & CHANNEL_JOIN) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit(player, chan->chan_obj, A_LOCK))
      return (1);
  }
  if ((flag_value & CHANNEL_TRANSMIT) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit(player, chan->chan_obj, A_LUSE))
      return (1);
  }
  if ((flag_value & CHANNEL_RECIEVE) &&
      !((chan->chan_obj == NOTHING) || (chan->chan_obj == 0))) {
    if (could_doit(player, chan->chan_obj, A_LENTER))
      return (1);
  }
  if (typeof_obj(player) == TYPE_PLAYER)
    flag_value *= CHANNEL_PL_MULT;
  else
    flag_value *= CHANNEL_OBJ_MULT;
  flag_value &= 0xFF; /*
                       * Mask out CHANNEL_PUBLIC and CHANNEL_LOUD
                       * just to be paranoid.
                       */

  return (((long)chan->type & flag_value));
}

int do_comsystem(DbRef who, char *cmd) {
  char *t;
  char *ch;
  char *alias;
  char *s;

  alias = alloc_lbuf("do_comsystem");
  s = alias;
  for (t = cmd; *t && *t != ' '; *s++ = *t++)
    /* nothing */;

  *s = '\0';

  if (*t)
    t++;

  ch = get_channel_from_alias(who, alias);
  if (ch && *ch) {
    do_processcom(who, ch, t);
    free_lbuf(alias);
    return 0;
  }
  free_lbuf(alias);
  return 1;
}

static void do_chclose(DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type &= (~(CHANNEL_PUBLIC));
  notify_printf(player, "@cset: Channel %s taken off the public listings.",
                chan);
  return;
}

void do_cemit(DbRef player, DbRef cause, int key, char *chan, char *text) {
  struct channel *ch;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(chan))) {
    notify_printf(player, "Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "Permission denied.");
    return;
  }
  if (key == CEMIT_NOHEADER)
    do_comsend(ch, text);
  else
    do_comprintf(ch, "[%s] %s", chan, text);
}

void do_chopen(DbRef player, DbRef cause, int key, char *chan, char *object) {
  struct channel *ch;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  switch (key) {
  case CSET_PRIVATE:
    do_chclose(player, chan);
    return;
  case CSET_LOUD:
    do_chloud(player, chan);
    return;
  case CSET_QUIET:
    do_chsquelch(player, chan);
    return;
  case CSET_LIST:
    do_chanlist(player, NOTHING, 1);
    return;
  case CSET_OBJECT:
    do_chanobj(player, chan, object);
    return;
  case CSET_STATUS:
    do_chanstatus(player, NOTHING, 1, chan);
    return;
  case CSET_TRANSPARENT:
    do_chtransparent(player, chan);
    return;
  case CSET_OPAQUE:
    do_chopaque(player, chan);
    return;
  }

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type |= (CHANNEL_PUBLIC);
  notify_printf(player, "@cset: Channel %s placed on the public listings.",
                chan);
  return;
}

static void do_chloud(DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type |= (CHANNEL_LOUD);
  notify_printf(player, "@cset: Channel %s now sends connect/disconnect msgs.",
                chan);
  return;
}

static void do_chsquelch(DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type &= ~(CHANNEL_LOUD);
  notify_printf(player, "@cset: Channel %s connect/disconnect msgs muted.",
                chan);
  return;
}

static void do_chtransparent(DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type |= CHANNEL_TRANSPARENT;
  notify_printf(player,
                "@cset: Channel %s now shows all listeners to everyone.", chan);
  return;
}

static void do_chopaque(DbRef player, char *chan) {
  struct channel *ch;

  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cset: Channel %s does not exist.", chan);
    return;
  }
  if ((player != ch->charge_who) && (!is_comm_all(player))) {
    raw_notify(player, "@cset: Permission denied.");
    return;
  }
  ch->type &= ~CHANNEL_TRANSPARENT;
  notify_printf(
      player, "@cset: Channel %s now does not show all listeners to everyone.",
      chan);
  return;
}

void do_chboot(DbRef player, DbRef cause, int key, char *channel,
               char *victim) {
  struct comuser *user;
  struct channel *ch;
  struct comuser *vu;
  DbRef thing;

  /*
   * * I sure hope it's not going to be that *
   * *  * *  * *  * * long.
   */

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  if (!(ch = select_channel(channel))) {
    raw_notify(player, "@cboot: Unknown channel.");
    return;
  }
  if (!(user = select_user(ch, player))) {
    raw_notify(player, "@cboot: You are not on that channel.");
    return;
  }
  if (!((ch->charge_who == player) || is_comm_all(player))) {
    raw_notify(player, "Permission denied.");
    return;
  }
  thing = match_thing(player, victim);

  if (thing == NOTHING) {
    return;
  }
  if (!(vu = select_user(ch, thing))) {
    notify_printf(player, "@cboot: %s in not on the channel.", Name(thing));
    return;
  }
  /*
   * We should be in the clear now. :)
   */
  do_comprintf(ch, "[%s] %s boots %s off the channel.", ch->name,
               unparse_object_numonly(player), unparse_object_numonly(thing));
  do_delcomchannel(thing, channel);
}

static void do_chanobj(DbRef player, char *channel, char *object) {
  struct channel *ch;
  DbRef thing;
  char *buff;

  init_match(player, object, NOTYPE);
  match_everything(0);
  thing = match_result();

  if (!(ch = select_channel(channel))) {
    raw_notify(player, "That channel does not exist.");
    return;
  }
  if (thing == NOTHING) {
    ch->chan_obj = NOTHING;
    raw_notify(player, "Set.");
    return;
  }
  if (!(ch->charge_who == player) && !is_comm_all(player)) {
    raw_notify(player, "Permission denied.");
    return;
  }
  ch->chan_obj = thing;
  buff = unparse_object(player, thing, 0);
  notify_printf(player, "Channel %s is now using %s as channel object.",
                ch->name, buff);
  free_lbuf(buff);
}

void do_chanlist(DbRef player, DbRef cause, int key) {
  DbRef owner;
  struct channel *ch;
  long flags;
  char *temp;
  char *buf;
  char *atrstr;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }
  flags = 0;

  if (key & CLIST_FULL) {
    do_listchannels(player);
    return;
  }
  temp = alloc_mbuf("do_chanlist_temp");
  buf = alloc_mbuf("do_chanlist_buf");

  raw_notify(player, "** Channel       Owner           Description");

  for (ch = (struct channel *)hash_table_first_entry(&mudstate.channel_htab);
       ch;
       ch = (struct channel *)hash_table_next_entry(&mudstate.channel_htab)) {
    if (is_comm_all(player) || (ch->type & CHANNEL_PUBLIC) ||
        ch->charge_who == player ||
        (do_test_access(player, CHANNEL_JOIN, ch))) {

      atrstr = attribute_parent_get(ch->chan_obj, A_DESC, &owner, &flags);
      if ((ch->chan_obj == NOTHING) || !*atrstr)
        snprintf(buf, MBUF_SIZE, "%s", "No description.");
      else
        snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr);

      free_lbuf(atrstr);
      snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-15.15s %-45.45s",
               (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
               (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
               Name(ch->charge_who), buf);

      raw_notify(player, temp);
    }
  }
  free_mbuf(temp);
  free_mbuf(buf);
  raw_notify(player, "-- End of list of Channels --");
}

void do_chanstatus(DbRef player, DbRef cause, int key, char *chan) {
  DbRef owner;
  struct channel *ch;
  long flags;
  char *temp;
  char *buf;
  char *atrstr;

  if (!mudconf.have_comsys) {
    raw_notify(player, "Comsys disabled.");
    return;
  }

  if (key & CSTATUS_FULL) {
    struct channel *selected_channel;
    int perm;

    if (!(perm = is_comm_all(player))) {
      raw_notify(
          player,
          "Warning: Only public channels and your channels will be shown.");
    }
    raw_notify(player, "** Channel             --Flags--  Obj   Own   Charge  "
                       "Balance  Users   Messages");

    if (!(selected_channel = select_channel(chan))) {
      notify_printf(player, "@cstatus: Channel %s does not exist.", chan);
      return;
    }
    if (perm || (selected_channel->type & CHANNEL_PUBLIC) ||
        selected_channel->charge_who == player) {

      notify_printf(
          player, "%c%c %-20.20s %c%c%c/%c%c%c %5d %5d %6d %10d",
          (selected_channel->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
          (selected_channel->type & (CHANNEL_LOUD)) ? 'L' : '-',
          selected_channel->name,
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_JOIN)) ? 'J'
                                                                      : '-',
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_TRANSMIT)) ? 'X'
                                                                          : '-',
          (selected_channel->type & (CHANNEL_PL_MULT * CHANNEL_RECIEVE)) ? 'R'
                                                                         : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_JOIN)) ? 'j'
                                                                       : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_TRANSMIT))
              ? 'x'
              : '-',
          (selected_channel->type & (CHANNEL_OBJ_MULT * CHANNEL_RECIEVE)) ? 'r'
                                                                          : '-',
          (selected_channel->chan_obj != NOTHING) ? selected_channel->chan_obj
                                                  : -1,
          selected_channel->charge_who, selected_channel->num_users,
          selected_channel->num_messages);
    }
    raw_notify(player, "-- End of list of Channels --");
    return;
  }
  temp = alloc_mbuf("do_chanstatus_temp");
  buf = alloc_mbuf("do_chanstatus_buf");

  raw_notify(player, "** Channel       Owner           Description");
  if (!(ch = select_channel(chan))) {
    notify_printf(player, "@cstatus: Channel %s does not exist.", chan);
    return;
  }
  if (is_comm_all(player) || (ch->type & CHANNEL_PUBLIC) ||
      ch->charge_who == player) {

    atrstr = attribute_parent_get(ch->chan_obj, A_DESC, &owner, &flags);
    if ((ch->chan_obj == NOTHING) || !*atrstr)
      snprintf(buf, MBUF_SIZE, "%s", "No description.");
    else
      snprintf(buf, MBUF_SIZE, "%-54.54s", atrstr);

    free_lbuf(atrstr);
    snprintf(temp, MBUF_SIZE, "%c%c %-13.13s %-15.15s %-45.45s",
             (ch->type & (CHANNEL_PUBLIC)) ? 'P' : '-',
             (ch->type & (CHANNEL_LOUD)) ? 'L' : '-', ch->name,
             Name(ch->charge_who), buf);

    raw_notify(player, temp);
  }
  free_mbuf(temp);
  free_mbuf(buf);
  raw_notify(player, "-- End of list of Channels --");
}

void fun_cemit(char *buff, char **bufc, DbRef player, DbRef cause,
               char *fargs[], int nfargs, char *cargs[], int ncargs) {
  struct channel *ch;

  if (!(ch = select_channel(fargs[0]))) {
    safe_str("#-1 CHANNEL NOT FOUND", buff, bufc);
    return;
  }

  if (!mudconf.have_comsys ||
      (!is_comm_all(player) && (player != ch->charge_who))) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }

  do_comprintf(ch, "[%s] %s", fargs[0], fargs[1]);
  *buff = '\0';
}
