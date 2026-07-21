
/*
 * player.c
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/password.h"
#include "mux/support/stringutil.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

// # of successful logins to save data for
constexpr int NUM_GOOD = 4;
// # of failed logins to save data for
constexpr int NUM_BAD = 3;

typedef struct hostdtm HOSTDTM;
struct hostdtm {
  char *host;
  char *dtm;
};

typedef struct logindata LDATA;
struct logindata {
  HOSTDTM good[NUM_GOOD];
  HOSTDTM bad[NUM_BAD];
  int tot_good;
  int tot_bad;
  int new_bad;
};

extern time_t time(time_t *);

/**
 * Decode login info.
 */
static void decrypt_logindata(char *atrbuf, LDATA *info) {
  int i;
  char *tmpc;

  info->tot_good = 0;
  info->tot_bad = 0;
  info->new_bad = 0;
  for (i = 0; i < NUM_GOOD; i++) {
    info->good[i].host = nullptr;
    info->good[i].dtm = nullptr;
  }
  for (i = 0; i < NUM_BAD; i++) {
    info->bad[i].host = nullptr;
    info->bad[i].dtm = nullptr;
  }

  if (*atrbuf == '#') {
    atrbuf++;
    if (!(tmpc = grabto(&atrbuf, ';')))
      return;
    info->tot_good = clamped_atoi(tmpc);
    for (i = 0; i < NUM_GOOD; i++) {
      if (!(tmpc = grabto(&atrbuf, ';')))
        return;
      info->good[i].host = tmpc;
      if (!(tmpc = grabto(&atrbuf, ';')))
        return;
      info->good[i].dtm = tmpc;
    }
    if (!(tmpc = grabto(&atrbuf, ';')))
      return;
    info->new_bad = clamped_atoi(tmpc);
    if (!(tmpc = grabto(&atrbuf, ';')))
      return;
    info->tot_bad = clamped_atoi(tmpc);
    for (i = 0; i < NUM_BAD; i++) {
      if (!(tmpc = grabto(&atrbuf, ';')))
        return;
      info->bad[i].host = tmpc;
      if (!(tmpc = grabto(&atrbuf, ';')))
        return;
      info->bad[i].dtm = tmpc;
    }
  }
}

/**
 * Encode login info.
 */
static void encrypt_logindata(char *atrbuf, LDATA *info) {
  char *bp, nullc;
  int i;

  /*
   * Make sure the SPRINTF call tracks NUM_GOOD and NUM_BAD for the * *
   *
   * *  * * number of host/dtm pairs of each type.
   */

  nullc = '\0';
  for (i = 0; i < NUM_GOOD; i++) {
    if (!info->good[i].host)
      info->good[i].host = &nullc;
    if (!info->good[i].dtm)
      info->good[i].dtm = &nullc;
  }
  for (i = 0; i < NUM_BAD; i++) {
    if (!info->bad[i].host)
      info->bad[i].host = &nullc;
    if (!info->bad[i].dtm)
      info->bad[i].dtm = &nullc;
  }
  bp = alloc_lbuf("encrypt_logindata");
  snprintf(
      bp, LBUF_SIZE, "#%d;%s;%s;%s;%s;%s;%s;%s;%s;%d;%d;%s;%s;%s;%s;%s;%s;",
      info->tot_good, info->good[0].host, info->good[0].dtm, info->good[1].host,
      info->good[1].dtm, info->good[2].host, info->good[2].dtm,
      info->good[3].host, info->good[3].dtm, info->new_bad, info->tot_bad,
      info->bad[0].host, info->bad[0].dtm, info->bad[1].host, info->bad[1].dtm,
      info->bad[2].host, info->bad[2].dtm);
  StringCopy(atrbuf, bp);
  free_lbuf(bp);
}

/**
 * Record successful or failed login attempt.
 * If successful, report the number of failures since the last successful
 * login.
 */
void record_login(EvaluationContext *evaluation, DbRef player, int isgood,
                  char *ldate, char *lhost, char *lusername) {
  LDATA login_info;
  char *atrbuf;
  DbRef aowner;
  long aflags;
  int i;

  atrbuf = attribute_get(evaluation->world->database, player, A_LOGINDATA,
                         &aowner, &aflags);
  decrypt_logindata(atrbuf, &login_info);
  if (isgood) {
    if (login_info.new_bad > 0) {
      notify(evaluation, player, "");
      notify_printf(
          evaluation, player,
          "**** %d failed connect%s since your last successful connect. ****",
          login_info.new_bad, (login_info.new_bad == 1 ? "" : "s"));
      notify_printf(evaluation, player,
                    "Most recent attempt was from %s on %s.",
                    login_info.bad[0].host, login_info.bad[0].dtm);
      notify(evaluation, player, "");
      login_info.new_bad = 0;
    }
    for (i = NUM_GOOD - 1; i > 0; i--) {
      login_info.good[i].dtm = login_info.good[i - 1].dtm;
      login_info.good[i].host = login_info.good[i - 1].host;
    }
    login_info.good[0].dtm = ldate;
    login_info.good[0].host = lhost;
    login_info.tot_good++;
    if (*lusername)
      attribute_add_raw(evaluation->world->database, player, A_LASTSITE,
                        tprintf("%s@%s", lusername, lhost));
    else
      attribute_add_raw(evaluation->world->database, player, A_LASTSITE, lhost);
  } else {
    for (i = NUM_BAD - 1; i > 0; i--) {
      login_info.bad[i].dtm = login_info.bad[i - 1].dtm;
      login_info.bad[i].host = login_info.bad[i - 1].host;
    }
    login_info.bad[0].dtm = ldate;
    login_info.bad[0].host = lhost;
    login_info.tot_bad++;
    login_info.new_bad++;
  }
  encrypt_logindata(atrbuf, &login_info);
  attribute_add_raw(evaluation->world->database, player, A_LOGINDATA, atrbuf);
  free_lbuf(atrbuf);
}

/**
 * Test a password to see if it is correct.
 */
int check_pass(WorldContext *world, DbRef player, const char *password) {
  DbRef aowner;
  long aflags;
  char *target;
  int matches;

  if (strlen(password) >
      (size_t)world->configuration->player_password_length_limit)
    return 0;
  target = attribute_get(world->database, player, A_PASS, &aowner, &aflags);
  matches = *target && password_verify(password, target);
  free_lbuf(target);
  return matches;
}

/**
 * Try to connect to an existing player.
 */
DbRef connect_player(EvaluationContext *evaluation, WorldContext *world,
                     char *name, char *password, char *host, char *username) {
  DbRef player;
  time_t tt;
  char *time_str;

  time(&tt);
  time_str = ctime(&tt);
  time_str[strlen(time_str) - 1] = '\0';

  if ((player = lookup_player(world, NOTHING, name, 0)) == NOTHING)
    return NOTHING;
  if (!check_pass(world, player, password)) {
    record_login(evaluation, player, 0, time_str, host, username);
    return NOTHING;
  }
  time(&tt);
  time_str = ctime(&tt);
  time_str[strlen(time_str) - 1] = '\0';

  attribute_add_raw(world->database, player, A_LAST, time_str);
  return player;
}

/**
 * Create a new player.
 */
DbRef create_player(EvaluationContext *evaluation, char *name, char *password,
                    DbRef creator, int isrobot) {
  WorldContext *world = evaluation->world;
  DbRef player;
  char hashed_password[crypto_pwhash_STRBYTES];
  char *pbuf;

  /*
   * Make sure the password is OK.  Name is checked in create_obj
   */

  if (!ok_new_player_name(world->configuration, name))
    return NOTHING;

  pbuf = trim_spaces(password);
  if (!ok_password(world->configuration, pbuf)) {
    free_lbuf(pbuf);
    return NOTHING;
  }
  if (!password_hash(world->configuration, pbuf, hashed_password)) {
    free_lbuf(pbuf);
    return NOTHING;
  }
  /*
   * If so, go create him
   */

  player = create_obj(evaluation, creator, TYPE_PLAYER, name);
  if (player == NOTHING) {
    sodium_memzero(hashed_password, sizeof(hashed_password));
    free_lbuf(pbuf);
    return NOTHING;
  }
  /*
   * initialize everything
   */
  /* comsys_add_alias()'s parameter isn't const-correct; "pub" is only read. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  if (*world->configuration->public_channel)
    comsys_add_alias(evaluation, player, (char *)"pub",
                     world->configuration->public_channel);
#pragma clang diagnostic pop

  object_password_set(world->database, player, hashed_password);
  game_object_set_link(world->database, player,
                       world->configuration->start_home != NOTHING
                           ? world->configuration->start_home
                           : world->configuration->start_room);
  s_fixed(world->database, player);
  sodium_memzero(hashed_password, sizeof(hashed_password));
  free_lbuf(pbuf);
  return player;
}

/**
 * Change the password for a player
 */
void do_password(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *oldpass = invocation->first;
  char *newpass = invocation->second;
  WorldContext *world = invocation->context->world;
  DbRef aowner;
  long aflags;
  char hashed_password[crypto_pwhash_STRBYTES];
  char *target;

  target = attribute_get(world->database, player, A_PASS, &aowner, &aflags);
  if (!*target || !check_pass(world, player, oldpass)) {
    notify(evaluation, player, "Sorry.");
  } else if (!ok_password(world->configuration, newpass)) {
    notify(evaluation, player, "Bad new password.");
  } else if (!password_hash(world->configuration, newpass, hashed_password)) {
    notify(evaluation, player, "Unable to change password.");
  } else {
    attribute_add_raw(world->database, player, A_PASS, hashed_password);
    sodium_memzero(hashed_password, sizeof(hashed_password));
    notify(evaluation, player, "Password changed.");
  }
  free_lbuf(target);
}

/**
 * Display login history data.
 */
static void disp_from_on(EvaluationContext *evaluation, DbRef player,
                         char *dtm_str, char *host_str) {
  if (dtm_str && *dtm_str && host_str && *host_str) {
    notify_printf(evaluation, player, "     From: %s   On: %s", dtm_str,
                  host_str);
  }
}

void do_last(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *who = invocation->first;
  WorldContext *world = invocation->context->world;
  DbRef target, aowner;
  LDATA login_info;
  char *atrbuf;
  int i;
  long aflags;

  if (!who || !*who || !(string_compare(world->configuration, who, "me"))) {
    target = game_object_owner(world->database, player);
  } else {
    target = lookup_player(world, player, who, 1);
  }

  if (target == NOTHING) {
    notify(evaluation, player, "I couldn't find that player.");
  } else if (!is_controls(evaluation, player, target)) {
    notify(evaluation, player, "Permission denied.");
  } else {
    atrbuf =
        attribute_get(world->database, target, A_LOGINDATA, &aowner, &aflags);
    decrypt_logindata(atrbuf, &login_info);

    notify_printf(&invocation->context->evaluation, player,
                  "Total successful connects: %d", login_info.tot_good);
    for (i = 0; i < NUM_GOOD; i++) {
      disp_from_on(&invocation->context->evaluation, player,
                   login_info.good[i].host, login_info.good[i].dtm);
    }
    notify_printf(&invocation->context->evaluation, player,
                  "Total failed connects: %d", login_info.tot_bad);
    for (i = 0; i < NUM_BAD; i++) {
      disp_from_on(&invocation->context->evaluation, player,
                   login_info.bad[i].host, login_info.bad[i].dtm);
    }
    free_lbuf(atrbuf);
  }
}

/*
 * add_player_name, delete_player_name, lookup_player:
 * Manage playername->dbref mapping
 */
int add_player_name(WorldContext *world, DbRef player, char *name) {
  int stat;
  DbRef *p;
  char *temp, *tp;

  /*
   * Convert to all lowercase
   */

  tp = temp = alloc_lbuf("add_player_name");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (tp = temp; *tp; tp++)
    *tp = ToLower(*tp);

  p = (long *)hash_table_find(temp, &world->indexes->players);
  if (p) {

    /*
     * Entry found in the hashtable.  If a player, succeed if the
     * * * numbers match (already correctly in the hash table),
     * fail * * if they don't.  Fail if the name is a disallowed
     * name * * (value AMBIGUOUS).
     */

    if (*p == AMBIGUOUS) {
      free_lbuf(temp);
      return 0;
    }
    if (is_good_obj(world->database, *p) &&
        (typeof_obj(world->database, *p) == TYPE_PLAYER)) {
      free_lbuf(temp);
      if (*p == player) {
        return 1;
      } else {
        return 0;
      }
    }
    /*
     * It's an alias (or an incorrect entry).  Clobber it
     */
    free(p);
    p = malloc(sizeof(DbRef));

    *p = player;
    stat = hash_table_replace(temp, p, &world->indexes->players);
    free_lbuf(temp);
  } else {
    p = malloc(sizeof(DbRef));

    *p = player;
    stat = hash_table_add(temp, p, &world->indexes->players);
    free_lbuf(temp);
    stat = (stat < 0) ? 0 : 1;
  }
  return stat;
}

int delete_player_name(WorldContext *world, DbRef player, char *name) {
  DbRef *p;
  char *temp, *tp;

  tp = temp = alloc_lbuf("delete_player_name");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (tp = temp; *tp; tp++)
    *tp = ToLower(*tp);

  p = (long *)hash_table_find(temp, &world->indexes->players);
  if (!p || (*p == NOTHING) || ((player != NOTHING) && (*p != player))) {
    free_lbuf(temp);
    return 0;
  }
  free(p);
  hash_table_delete(temp, &world->indexes->players);
  free_lbuf(temp);
  return 1;
}

DbRef lookup_player(WorldContext *world, DbRef doer, char *name,
                    int check_who) {
  DbRef *p, thing;
  char *temp, *tp;

  if (!string_compare(world->configuration, name, "me"))
    return doer;

  if (*name == NUMBER_TOKEN) {
    name++;
    if (!is_number(name))
      return NOTHING;
    thing = clamped_atol(name);
    if (!is_good_obj(world->database, thing))
      return NOTHING;
    if (!((typeof_obj(world->database, thing) == TYPE_PLAYER) ||
          is_god(world->database, doer)))
      thing = NOTHING;
    return thing;
  }
  tp = temp = alloc_lbuf("lookup_player");
  safe_str(name, temp, &tp);
  *tp = '\0';
  for (tp = temp; *tp; tp++)
    *tp = ToLower(*tp);
  p = (long *)hash_table_find(temp, &world->indexes->players);
  free_lbuf(temp);
  if (!p) {
    if (check_who) {
      thing =
          find_connected_name(world->database, world->descriptors, doer, name);
      if (is_dark(world->database, thing))
        thing = NOTHING;
    } else
      thing = NOTHING;
  } else if (!is_good_obj(world->database, *p)) {
    thing = NOTHING;
  } else
    thing = *p;

  return thing;
}

void load_player_names(WorldContext *world) {
  DbRef i, aowner;
  long aflags;
  char *alias;

  DO_WHOLE_DB(world->database, i) {
    if (typeof_obj(world->database, i) == TYPE_PLAYER) {
      add_player_name(world, i, game_object_name(world->database, i));
    }
  }
  alias = alloc_lbuf("load_player_names");
  DO_WHOLE_DB(world->database, i) {
    if (typeof_obj(world->database, i) == TYPE_PLAYER) {
      alias = attribute_get_string(world->database, alias, i, A_ALIAS, &aowner,
                                   &aflags);
      if (*alias)
        add_player_name(world, i, alias);
    }
  }
  free_lbuf(alias);
}

/**
 * badname_add, badname_check, badname_list: Add/look for/display bad names.
 */
void badname_add(WorldContext *world, char *bad_name) {
  BADNAME *bp;

  /*
   * Make a new node and link it in at the top
   */

  bp = (BADNAME *)malloc(sizeof(BADNAME));
  bp->name = malloc(strlen(bad_name) + 1);
  bp->next = world->access_control->bad_names;
  world->access_control->bad_names = bp;
  StringCopy(bp->name, bad_name);
}

void badname_remove(WorldContext *world, char *bad_name) {
  BADNAME *bp, *backp;

  /*
   * Look for an exact match on the bad name and remove if found
   */

  backp = nullptr;
  for (bp = world->access_control->bad_names; bp; backp = bp, bp = bp->next) {
    if (!string_compare(world->configuration, bad_name, bp->name)) {
      if (backp)
        backp->next = bp->next;
      else
        world->access_control->bad_names = bp->next;
      free(bp->name);
      free(bp);
      return;
    }
  }
}

int badname_check(WorldContext *world, char *bad_name) {
  BADNAME *bp;

  /*
   * Walk the badname list, doing wildcard matching.  If we get a hit *
   *
   * *  * *  * * then return false.  If no matches in the list, return
   * true.
   */

  for (bp = world->access_control->bad_names; bp; bp = bp->next) {
    if (quick_wild(bp->name, bad_name))
      return 0;
  }
  return 1;
}

void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *prefix) {
  BADNAME *bp;
  char *buff, *bufp;

  /*
   * Construct an lbuf with all the names separated by spaces
   */

  buff = bufp = alloc_lbuf("badname_list");
  safe_str(prefix, buff, &bufp);
  for (bp = world->access_control->bad_names; bp; bp = bp->next) {
    safe_chr(' ', buff, &bufp);
    safe_str(bp->name, buff, &bufp);
  }
  *bufp = '\0';

  /*
   * Now display it
   */

  notify(evaluation, player, buff);
  free_lbuf(buff);
}
