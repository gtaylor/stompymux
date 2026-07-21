/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 */

#include "mux/server/platform.h"
#include <math.h>
#include <stdio.h>
#define BTECHSTATS
#include "coolmenu.h"
#include "mech.events.h"
#include "mech.h"
#include "mycool.h"
#define BTECHSTATS_C
#include "btechstats.h"
#include "btmacros.h"
#include "glue.h"
#include "mux/commands/command_helpers.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_runtime.h"
#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/support/hash_table.h"
#include "p.bsuit.h"
#include "p.map.obj.h"
#include "p.mech.combat.h"
#include "p.mech.combat.misc.h"
#include "p.mech.partnames.h"
#include "p.mech.pickup.h"
#include "p.mech.tag.h"
#include "p.mech.update.h"
#include "p.mech.utils.h"
#include "p.mechfile.h"

static int char_xp_bonus(PSTATS *s, int code);
static int char_getstatvalue(PSTATS *s, char *name);
static void char_setstatvalue(PSTATS *s, char *name, int value);
static void retrieve_stats(BtechContext *context, DbRef player, int modes,
                           PSTATS *stats);
static void clear_player(PSTATS *s);
static void store_stats(BtechContext *context, DbRef player, PSTATS *s,
                        int modes);

UptimeText uptime_text(int seconds) {
  UptimeText uptime;
  char *allocated;

  allocated = get_uptime_to_string(seconds);
  snprintf(uptime.text, sizeof(uptime.text), "%s", allocated);
  free_sbuf(allocated);
  return uptime;
}

static int char_getskilltargetbycode_base(BtechContext *context, DbRef player,
                                          PSTATS *s, int code, int modifier,
                                          int use_xp);

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier);

static int figure_xp_bonus(BtechContext *context, DbRef player, PSTATS *s,
                           int code) {
  int t = char_values[code].xpthreshold;
  int tx, bon, btar;

  if (t <= 0)
    return 0;
  /* KLUDGE */
  s->xp[code] = s->xp[code] %
                XP_MAX; /* reset exp modifier - this probably _was_ cached */
  btar = char_getskilltargetbycode_base(context, player, s, code, 0, 0);
  while (btar > 4) {
    btar--;
    t = t / 3;
  }
  while (btar < 4) {
    btar++;
    t = t * 3;
  }
  if (t < 1)
    t = 1;
  tx = s->xp[code] % XP_MAX;
  bon = 0;
  while (tx > t) {
    bon++;
    tx -= t;
    t = t * 3;
  }
  return bon;
}

static int figure_xp_to_next_level(BtechContext *context, DbRef target,
                                   int code) {
  int xpthresh = char_values[code].xpthreshold;
  int start_skill, target_skill, counter, running_total = 1;

  if (xpthresh <= 0)
    return -1;
  target_skill = char_getskilltargetbycode(context, target, code, 0);
  start_skill = char_getskilltargetbycode_noxp(context, target, code, 0);
  counter = start_skill;
  while (counter > 4) {
    counter--;
    xpthresh /= 3;
  }
  while (counter < 4) {
    counter++;
    xpthresh *= 3;
  }
  if (xpthresh < 1)
    xpthresh = 1;
  while (target_skill <= start_skill) {
    start_skill--;
    running_total += xpthresh;
    xpthresh *= 3;
  }
  return running_total;
}

/* Right now applies to only very few select skills */

static int char_xp_bonus(PSTATS *s, int code) { return s->xp[code] / XP_MAX; }

/*****************************/

/*     list commands        */

/*****************************/

void list_charvaluestuff(EvaluationContext *evaluation, DbRef player,
                         int flag) {
  int found = 0, ok, type;
  int i;
  char buf[80] = {0};

  if (flag == -1)
    notify(evaluation, player, "List of charvalues available:");
  if (flag >= 0) {
    notify_printf(evaluation, player,
                  "List of %s available:", btech_charvaluetype_names[flag]);
  }
  buf[0] = 0;
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    ok = 0;
    type = char_values[i].type;
    if (flag < 0)
      ok = 1;
    else if (type == flag)
      ok = 1;
    if (ok) {
      snprintf(buf + strlen(buf), 80 - strlen(buf), "%-23s ",
               char_values[i].name);
      if (!((++found) % 3)) {
        notify(evaluation, player, buf);
        strcpy(buf, " ");
      }
    }
  }
  if (found % 3) {
    notify(evaluation, player, buf);
  }
  notify(evaluation, player, " ");
  notify_printf(evaluation, player, "Total of %d things found.", found);
}

/*****************************/

/*     get code commands    */

/*****************************/

int char_getvaluecode(BtechContext *context, char *name) {
  int *ip;
  char *tmpbuf, *tmpc1, *tmpc2;

  tmpbuf = alloc_sbuf("getvaluecodefind");
  for (tmpc1 = name, tmpc2 = tmpbuf;
       *tmpc1 && ((tmpbuf - tmpc2) < (SBUF_SIZE - 1)); tmpc1++, tmpc2++)
    *tmpc2 = ToLower(*tmpc1);
  *tmpc2 = 0;
  if ((ip = hash_table_find(tmpbuf, &context->player_value_hashes[0])) == NULL)
    ip = hash_table_find(tmpbuf, &context->player_value_hashes[1]);
  free_sbuf(tmpbuf);
  return ((long)ip) - 1;
}

/********************/

/*   Roll the dice  */

/********************/

int char_rollsaving(BtechContext *context) {
  int r1, r2, r3;
  int r12, r13, r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 > r13) {
    if (r12 > r23)
      return r12;
    else
      return r23;
  } else {
    if (r13 > r23)
      return r13;
    else
      return r23;
  }
}

int char_rollunskilled(BtechContext *context) {
  int r1, r2, r3;
  int r12, r13, r23;

  r1 = char_rolld6(context, 1);
  r2 = char_rolld6(context, 1);
  r3 = char_rolld6(context, 1);

  r12 = r1 + r2;
  r13 = r1 + r3;
  r23 = r2 + r3;

  if (r12 < r13) {
    if (r12 < r23)
      return r12;
    else
      return r23;
  } else {
    if (r13 < r23)
      return r13;
    else
      return r23;
  }
}

int char_rollskilled(BtechContext *context) { return char_rolld6(context, 2); }

int char_rolld6(BtechContext *context, int num) {
  int i, total = 0;

  for (i = 0; i < num; i++)
    total = total + btech_random_range(context, 1, 6);
  return (total);
}

/*****************************/

/*     DB access commands   */

/*****************************/

static int char_getstatvalue(PSTATS *s, char *name) {
  for (size_t i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(char_values[i].name, name))
      return char_getstatvaluebycode(s, i);
  return -1;
}

static void char_setstatvalue(PSTATS *s, char *name, int value) {
  for (size_t i = 0; i < NUM_CHARVALUES; i++)
    if (!strcasecmp(char_values[i].name, name)) {
      char_setstatvaluebycode(s, i, value);
      return;
    }
}

static int char_getvaluebycode(BtechContext *context, DbRef player, int code) {
  PSTATS stats;

  retrieve_stats(context, player, VALUES_ALL, &stats);
  return char_getstatvaluebycode((&stats), code);
}

static void char_setvaluebycode(BtechContext *context, DbRef player, int code,
                                int value) {
  PSTATS stats;

  retrieve_stats(context, player, VALUES_ALL, &stats);
  char_setstatvaluebycode((&stats), code, value);
  store_stats(context, player, &stats, VALUES_ALL);
}

int char_getvalue(BtechContext *context, DbRef player, char *name) {
  return char_getvaluebycode(context, player, char_getvaluecode(context, name));
}

void char_setvalue(BtechContext *context, DbRef player, char *name, int value) {
  char_setvaluebycode(context, player, char_getvaluecode(context, name), value);
}

static int char_getskilltargetbycode_base(BtechContext *context, DbRef player,
                                          PSTATS *s, int code, int modifier,
                                          int use_xp) {
  int val, skill;

  if (code == -1)
    return 18;
  if (char_values[code].type != CHAR_SKILL)
    return 18;
  if (use_xp && context->cached_target_character == player &&
      context->cached_skill == code)
    return context->cached_skill_result + modifier;
  if (char_values[code].flag & CHAR_ATHLETIC)
    val = char_gvalue(s, "build") + char_gvalue(s, "reflexes");
  else if (char_values[code].flag & CHAR_PHYSICAL)
    val = char_gvalue(s, "reflexes") + char_gvalue(s, "intuition");
  else if (char_values[code].flag & CHAR_MENTAL)
    val = char_gvalue(s, "intuition") + char_gvalue(s, "learn");
  else if (char_values[code].flag & CHAR_PHYSICAL)
    val = char_gvalue(s, "reflexes") + char_gvalue(s, "intuition");
  else if (char_values[code].flag & CHAR_SOCIAL)
    val = char_gvalue(s, "intuition") + char_gvalue(s, "charisma");
  else
    return 18;
  if (use_xp) {
    skill = char_getstatvaluebycode(s, code);

    if (skill == -1)
      return 18;
    context->cached_target_character = player;
    context->cached_skill = code;
    context->cached_skill_result = 18 - val - skill;
    return context->cached_skill_result + modifier;
  } else {
    skill = s->values[code];
    if (skill == -1)
      return (18);
    return 18 - val - skill;
  }
}

int char_getskilltargetbycode(BtechContext *context, DbRef player, int code,
                              int modifier) {
  PSTATS stats, *s = &stats;

  retrieve_stats(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(context, player, s, code, modifier, 1);
}

static int char_getskilltargetbycode_noxp(BtechContext *context, DbRef player,
                                          int code, int modifier) {
  PSTATS stats, *s = &stats;

  retrieve_stats(context, player, VALUES_CO, s);
  return char_getskilltargetbycode_base(context, player, s, code, modifier, 0);
}

int char_getskilltarget(BtechContext *context, DbRef player, char *name,
                        int modifier) {
  return char_getskilltargetbycode(context, player,
                                   char_getvaluecode(context, name), modifier);
}

int char_getxpbycode(BtechContext *context, DbRef player, int code) {
  PSTATS stats, *s = &stats;

  if (code < 0)
    return 0;
  retrieve_stats(context, player, VALUES_SKILLS, s);
  return s->xp[code] % XP_MAX;
}

int char_gainxpbycode(BtechContext *context, DbRef player, int code, int amount,
                      int override) {
  PSTATS stats, *s = &stats;

  if (code < 0)
    return 0;
  retrieve_stats(context, player, VALUES_SKILLS | VALUES_ATTRS, s);
  /* allow override of setting xp quickly. useful in chargen situations and only
   * settable via that Regular skill gains still check SK_XP and last used
   * within 30s to keep from spamming
   */
  if (override == 0)
    if (!((context->clock->now > (s->last_use[code] + 30)) ||
          (char_values[code].flag & SK_XP)))
      return 0;
  s->last_use[code] = context->clock->now;
  s->xp[code] += amount;
  s->xp[code] =
      s->xp[code] % XP_MAX + XP_MAX * figure_xp_bonus(context, player, s, code);
  store_stats(context, player, s, VALUES_SKILLS);
  return 1;
}

int char_gainxp(BtechContext *context, DbRef player, char *skill, int amount) {
  return char_gainxpbycode(context, player, char_getvaluecode(context, skill),
                           amount, 0);
}

int char_getskillsuccess(BtechContext *context, DbRef player, char *name,
                         int modifier, int loud) {
  int roll, val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, modifier);

  if (char_getvaluebycode(context, player, code) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);
  if (loud) {
    notify_printf(btech_context_evaluation(context), player,
                  "You make a %s skill roll!", name);
    notify_printf(btech_context_evaluation(context), player,
                  "Modified skill BTH : %d Roll : %d", val, roll);
  }

  if (roll >= val)
    return (1); /* Success! */
  else
    return (0); /* Failure */
}

int char_getskillmargsucc(BtechContext *context, DbRef player, char *name,
                          int modifier) {
  int roll, val;
  int code;

  code = char_getvaluecode(context, name);

  val = char_getskilltargetbycode(context, player, code, modifier);

  if (char_getvaluebycode(context, player, code) == 0)
    roll = char_rollunskilled(context);
  else
    roll = char_rollskilled(context);

  return (roll - val);
}

int char_getopposedskill(BtechContext *context, DbRef first, char *skill1,
                         DbRef second, char *skill2) {
  int per1, per2;

  per1 = char_getskillmargsucc(context, first, skill1, 0);
  per2 = char_getskillmargsucc(context, second, skill2, 0);

  if (per1 > per2)
    return (first);
  else if (per2 == per1)
    return (0);
  else
    return (second);
}

int char_getattrsave(BtechContext *context, DbRef player, char *name) {
  int val = char_getvalue(context, player, name);

  if (val == -1)
    return (-1);
  else if (val > 9)
    return 0;
  else
    return (18 - 2 * val);
}

int char_getattrsavesucc(BtechContext *context, DbRef player, char *name) {
  int roll, val = char_getattrsave(context, player, name);

  if (val == -1)
    return (-1);

  roll = char_rollskilled(context);

  if (roll >= val)
    return (1);
  else
    return (0);
}

/************************/

/*    Database Commands */

/************************/

void init_btechstats(BtechContext *context) {
  char *tmpbuf, *tmpc1, *tmpc2;
  long i;
  int j;

  context->player_value_hashes =
      calloc(2, sizeof(*context->player_value_hashes));
  context->char_value_short_names =
      calloc(NUM_CHARVALUES, sizeof(*context->char_value_short_names));
  if (context->player_value_hashes == nullptr ||
      context->char_value_short_names == nullptr)
    exit(EXIT_FAILURE);
  context->char_value_count = NUM_CHARVALUES;
  hash_table_initialize(&context->player_value_hashes[0], 20 * HASH_FACTOR);
  hash_table_initialize(&context->player_value_hashes[1], 20 * HASH_FACTOR);
  tmpbuf = alloc_sbuf("getvaluecode");
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    for (tmpc1 = char_values[i].name, tmpc2 = tmpbuf; *tmpc1; tmpc1++, tmpc2++)
      *tmpc2 = ToLower(*tmpc1);
    *tmpc2 = '\0';
    hash_table_add(tmpbuf, (int *)(i + 1), &context->player_value_hashes[0]);
    tmpbuf[0] = '\0';
    tmpc1 = tmpbuf;
    for (j = 0; char_values[i].name[j]; j++) {
      if (!isupper(char_values[i].name[j]))
        continue;
      strncpy(tmpc1, &char_values[i].name[j], 3);
      tmpc1 += 3;
    }
    *tmpc1 = '\0';
    if (strlen(tmpbuf) <= 3) {
      strncpy(tmpbuf, char_values[i].name, 5);
      tmpbuf[5] = '\0';
    }
    context->char_value_short_names[i] = strdup(tmpbuf);
    for (tmpc1 = tmpbuf; *tmpc1; tmpc1++)
      *tmpc1 = ToLower(*tmpc1);
    hash_table_add(tmpbuf, (int *)(i + 1), &context->player_value_hashes[1]);
  }
  free_sbuf(tmpbuf);
}

void btech_stats_destroy(BtechContext *context) {
  if (context == nullptr)
    return;

  if (context->player_value_hashes != nullptr) {
    hash_table_destroy(&context->player_value_hashes[0]);
    hash_table_destroy(&context->player_value_hashes[1]);
    free(context->player_value_hashes);
    context->player_value_hashes = nullptr;
  }
  for (size_t i = 0; i < context->char_value_count; i++)
    free(context->char_value_short_names[i]);
  free(context->char_value_short_names);
  context->char_value_short_names = nullptr;
  context->char_value_count = 0;
  context->cached_target_character = -1;
}

static PSTATS *create_new_stats(void) {
  PSTATS *s;

  Create(s, PSTATS, 1);
  s->DbRef = -1;
  clear_player(s);
  return s;
}

static void clear_player(PSTATS *s) {
  int i;

  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    s->values[i] = (char_values[i].type == CHAR_ATTRIBUTE ? 1 : 0);
    s->xp[i] = 0;
  }
  char_slives(s, 1);
}

/************************/

/*      MUSE COMMANDS   */

/************************/

void do_charclear(CommandInvocation *invocation) {
  CommandContext *command = invocation->context;
  GameDatabase *database = command->world->database;
  DbRef player = invocation->player;

  if (!invocation->first || !*invocation->first) {
    notify(&command->evaluation, player,
           "Who do you want to clear the stats from?");
    return;
  }

  DbRef thing = lookup_player(command->world, player, invocation->first, 0);
  if (thing == NOTHING) {
    notify(&command->evaluation, player, "I don't know who that is");
    return;
  }

  silly_atr_set_in(database, thing, A_ATTRS, "");
  silly_atr_set_in(database, thing, A_SKILLS, "");
  silly_atr_set_in(database, thing, A_ADVS, "");
  silly_atr_set_in(database, thing, A_HEALTH, "");
  notify_printf(&command->evaluation, player, "Player #%ld stats cleared",
                thing);
}

DbRef char_lookupplayer(BtechContext *context, DbRef player, DbRef cause,
                        int key, char *arg1) {
  WorldContext world = {
      .database = context->database,
      .configuration = context->configuration,
      .indexes = context->world_indexes,
      .access_control = context->access_control,
  };
  return lookup_player(&world, player, arg1, 0);
}

static int loc_mod(int loc) {
  switch (loc) {
  case HEAD:
    return 15;
  case CTORSO:
    return 50;
  case LTORSO:
  case RTORSO:
    return 35;
  case LARM:
  case RARM:
    return 30;
  case LLEG:
  case RLEG:
    return 35;
  }
  return 0;
}

void initialize_pc(DbRef player, MECH *mech) {
  BtechContext *context = mech->xcode.context;
  PSTATS stats, *s = &stats;
  int bruise, lethal, playerBLD;
  int dam, tot;
  char *c;
  int cnt;
  char buf1[MBUF_SIZE];
  char buf2[MBUF_SIZE];
  char buf3[MBUF_SIZE];
  char buf4[2];
  int ammo1;
  int ammo2;
  int i, id, brand;
  int pc_loc_to_mech_loc[] = {HEAD, CTORSO, RARM, RLEG};

  if (!(MechType(mech) == CLASS_MW && !(MechCritStatus(mech) & PC_INITIALIZED)))
    return;
  buf4[1] = 0;
  retrieve_stats(context, player, VALUES_HEALTH | VALUES_ATTRS | VALUES_SKILLS,
                 s);
  playerBLD = char_gvalue(s, "build");
  MechCritStatus(mech) |= PC_INITIALIZED;
  bruise = char_gbruise(s);
  lethal = char_glethal(s);
  tot = playerBLD * 20;
  dam = bruise + lethal;
  MechMaxSpeed(mech) =
      (playerBLD + char_gvalue(s, "reflexes") + char_gvalue(s, "running")) *
      MP1 / 9.0;
#define PC_LOCS 4
  for (i = 0; i < NUM_SECTIONS; i++) {
    SetSectArmor(mech, i, 0);
    SetSectOArmor(mech, i, 0);
    SetSectInt(mech, i, (loc_mod(i) * (tot - dam)) / 100 + 1);
    SetSectOInt(mech, i, (loc_mod(i) * (tot - dam)) / 100 + 1);
  }
  c = btech_attribute_read(context->database, player, A_PCEQUIP,
                           (char[LBUF_SIZE]){0});
  cnt = sscanf(c, "%s %s %s %d %d", buf1, buf2, buf3, &ammo1, &ammo2);

  switch (cnt) {
  case 5:
  case 4:
  case 3:
    if (strcmp(buf3, "-")) {
      if (!find_matching_vlong_part(context, buf3, nullptr, &id, &brand)) {
        SendError(context, tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                                   game_object_name(
                                       mech->xcode.context->database, player),
                                   player, buf3));
        return;
      }
      if (IsWeapon(id)) {
        SetPartType(mech, LARM, 0, id);
        SetPartData(mech, LARM, 0, 0);
        SetPartFireMode(mech, LARM, 0, 0);
        SetPartAmmoMode(mech, LARM, 0, 0);
        if ((i = MechWeapons[Weapon2I(id)].ammoperton)) {
          SetPartType(mech, LARM, 1, I2Ammo(Weapon2I(id)));
          SetPartData(mech, LARM, 1, cnt >= 5 ? ammo2 : i);
          SetPartFireMode(mech, LARM, 1, 0);
          SetPartAmmoMode(mech, LARM, 1, 0);
        }
      }
    }
    [[fallthrough]];
  case 2:
    if (strcmp(buf2, "-")) {
      if (!find_matching_vlong_part(context, buf2, nullptr, &id, &brand)) {
        SendError(context, tprintf("Invalid PC weapon #1 for %s(#%ld): %s",
                                   game_object_name(
                                       mech->xcode.context->database, player),
                                   player, buf2));
        return;
      }
      if (IsWeapon(id)) {
        SetPartType(mech, RARM, 0, id);
        SetPartData(mech, RARM, 0, 0);
        SetPartFireMode(mech, RARM, 0, 0);
        SetPartAmmoMode(mech, RARM, 0, 0);
        if ((i = MechWeapons[Weapon2I(id)].ammoperton)) {
          SetPartType(mech, RARM, 1, I2Ammo(Weapon2I(id)));
          SetPartData(mech, RARM, 1, cnt >= 4 ? ammo1 : i);
          SetPartFireMode(mech, RARM, 1, 0);
          SetPartAmmoMode(mech, RARM, 1, 0);
        }
      }
    }
    [[fallthrough]];
  case 1:
    if (strlen(buf1) != PC_LOCS) {
      SendError(context,
                tprintf("Invalid armor string for %s(#%ld): %s",
                        game_object_name(mech->xcode.context->database, player),
                        player, buf1));
      return;
    }
    for (i = 0; buf1[i]; i++)
      if (!isdigit(buf1[i])) {
        SendError(
            context,
            tprintf("Invalid armor char for %s(#%ld) in %s (pos %d,%c)",
                    game_object_name(mech->xcode.context->database, player),
                    player, buf1, i + 1, buf1[i]));
        return;
      }
    for (i = 0; buf1[i]; i++) {
      buf4[0] = buf1[i];
      SetSectArmor(mech, pc_loc_to_mech_loc[i], atoi(buf4));
    }
  }
}

void fix_pilotdamage(MECH *mech, DbRef player) {
  BtechContext *context = mech->xcode.context;
  PSTATS stats, *s = &stats;
  int bruise, lethal, playerBLD;

  retrieve_stats(context, player, VALUES_HEALTH | VALUES_ATTRS, s);
  bruise = char_gbruise(s);
  lethal = char_glethal(s);
  playerBLD = char_gvalue(s, "build") * 2;
  if (playerBLD < 1 || playerBLD > 100)
    playerBLD = 10;

  MechPilotStatus(mech) = (bruise + lethal) / playerBLD;
}

const int PilotStatusRollNeeded[] = {0, 3, 5, 7, 10, 11};

#define CHDAM(val, ret)                                                        \
  if (playerhits >= ((val)))                                                   \
    return ret * mod;

int mw_ic_bth(MECH *mech) {
  BtechContext *context = mech->xcode.context;
  /* Rule Reference: BMR Revised, Page 17 ( Consciousness Table ) */
  /* Rule Reference: Total Warfare, Page 41-42 ( Consciousness Table ) */
  /* Rule Reference: MaxTech Revised, Page 46 ( Pain Resistance = -1 ) */

  int playerBLD;
  int bruise, playerhits;
  PSTATS stats, *s = &stats;
  int mod = 0;

  retrieve_stats(context, MechPilot(mech),
                 VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH, s);
  playerBLD = char_gvalue(s, "build");
  bruise = char_gbruise(s);
  playerhits = 10 * playerBLD - bruise;
  if (char_gvalue(s, "pain_resistance") == 1)
    mod = -1;
  if (playerhits >= (8 * playerBLD))
    return 3 + mod;
  else if (playerhits >= (6 * playerBLD))
    return 5 + mod;
  else if (playerhits >= (4 * playerBLD))
    return 7 + mod;
  else if (playerhits >= (2 * playerBLD))
    return 10 + mod;
  else if (playerhits >= -1)
    return 11 + mod;
  return 0;
}

int handlemwconc(MECH *mech, int initial) {
  /* Rule Reference: MechWarrior 2nd Edition RPG, Page 22 (Toughness = Best of
   * 3D6) */
  /* Rule Reference: Old Tactical Handbook, Page 51 (Use MW 2nd Edition) */
  /* Rule Reference: BMR Revised, Page 17 ( >5 Bruise = Death ) */
  /* Rule Reference: Total Warfare, Page 41-42 ( >5 Bruise = Death ) */

  int m, roll;

  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      MechPilot(mech) > 0)
    m = mw_ic_bth(mech);
  else {
    if (initial)
      if (MechPilotStatus(mech) > 5) {
        mech_notify(mech, MECHPILOT, "You are killed from personal injuries!!");

        // This is here to avoid multi-triggers of AMECHDEST.
        if (!Destroyed(mech))
          DestroyMech(mech, mech, 0, KILL_TYPE_MWDAMAGE);

        MechPilot(mech) = -1;
        MechSpeed(mech) = 0.;
        MechDesiredSpeed(mech) = 0.;
        return 0;
      }
    m = PilotStatusRollNeeded[BOUNDED(0, (int)MechPilotStatus(mech), 4)];
  }
  if (initial && Uncon(mech))
    return 0;
  if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "toughness"))
    /*  Gets the saving roll for someone with toughness  */
    roll = char_rollsaving(mech->xcode.context);
  else
    roll = char_rollskilled(mech->xcode.context);
  if (MechPilot(mech) >= 0) {
    if (initial) {
      mech_notify(mech, MECHPILOT, "You attempt to keep consciousness!");
      mech_printf(mech, MECHPILOT, "Retain Conciousness on: %d  \tRoll: %d",
                  abs(m), roll);
    } else {
      mech_notify(mech, MECHPILOT, "You attempt to regain consciousness!");
      mech_printf(mech, MECHPILOT, "Regain Consciousness on: %d  \tRoll: %d",
                  abs(m), roll);
    }
  }
  if (roll < (abs(m))) {
    if (initial)
      mech_notify(mech, MECHPILOT,
                  "Consciousness slips away from you as you enter a sea of "
                  "darkness...");
    ProlongUncon(mech, UNCONSCIOUS_TIME);
    return 0;
  }
  return 1;
}

void headhitmwdamage(MECH *mech, MECH *attacker, int dam) {
  BtechContext *context = mech->xcode.context;
  PSTATS stats, *s = &stats;
  DbRef player;
  int damage, bruise, lethaldam, playerBLD;

  if (mech->mynum < 0)
    return;
  /* check to see if mech is IC */
  if (!is_in_character(mech->xcode.context->database, mech->mynum) ||
      !GotPilot(mech)) {
    MechPilotStatus(mech) += dam;
    handlemwconc(mech, 1);
    return;
  }
  player = MechPilot(mech);

  retrieve_stats(context, player, VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH,
                 s);
  /* get the player_stats structure */

  bruise = char_gbruise(s);
  /* gets the players bruise damage */

  playerBLD = char_gvalue(s, "build");
  /* get the player's BLD value */

  damage = 2 * playerBLD * dam;
  /* the damage we are due */

  bruise += damage;
  /* this part subtracts 10 from players lethal damage */

  if (bruise > playerBLD * 10) {
    lethaldam = char_glethal(s);
    lethaldam += (bruise - playerBLD * 10);
    bruise = playerBLD * 10;

    if (lethaldam >= playerBLD * 10) {
      lethaldam = playerBLD * 10;
      char_slethal(s, playerBLD * 10 - 1);
      char_sbruise(s, playerBLD * 10);
      store_stats(context, player, s, VALUES_HEALTH);
      if (!Destroyed(mech)) {
        DestroyMech(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
      }
      KillMechContentsIfIC(mech);
      return;
    }
    char_slethal(s, lethaldam);
  }
  char_sbruise(s, bruise);
  store_stats(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  MechPilotStatus(mech) += dam;
}

void mwlethaldam(MECH *mech, MECH *attacker, int dam) {
  BtechContext *context = mech->xcode.context;
  PSTATS stats, *s = &stats;
  DbRef player;
  int lethaldam, playerBLD;

  if (mech->mynum < 0)
    return;
  /* check to see if mech is IC */
  if (!is_in_character(mech->xcode.context->database, mech->mynum) ||
      !GotPilot(mech)) {
    MechPilotStatus(mech) += dam;
    handlemwconc(mech, 1);
    return;
  }
  player = MechPilot(mech);

  retrieve_stats(context, player, VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH,
                 s);
  /* get the player_stats structure */
  playerBLD = char_gvalue(s, "build");
  if (!playerBLD)
    playerBLD++;
  lethaldam = char_glethal(s);
  lethaldam += BOUNDED(10, dam * playerBLD, 40);
  if (lethaldam >= playerBLD * 10) {
    lethaldam = playerBLD * 10;
    char_slethal(s, lethaldam - 1);
    char_sbruise(s, lethaldam);
    store_stats(context, player, s, VALUES_HEALTH);
    if (!Destroyed(mech)) {
      DestroyMech(mech, attacker, 0, KILL_TYPE_MWDAMAGE);
    }
    KillMechContentsIfIC(mech);
    return;
  }
  char_sbruise(s, playerBLD * 10 - 5);
  char_slethal(s, lethaldam);
  store_stats(context, player, s, VALUES_HEALTH);
  handlemwconc(mech, 1);
  MechPilotStatus(mech) += dam;
}

void lower_xp(BtechContext *context, DbRef player, int promillage) {
  PSTATS stats, *s = &stats;
  int i;

  retrieve_stats(context, player, VALUES_ALL, s);
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    if (!s->xp[i])
      continue;
    if (s->xp[i] < 0) {
      s->xp[i] = 0;
      continue;
    }
    s->xp[i] = (s->xp[i] % XP_MAX) * promillage / 1000;
    s->xp[i] =
        s->xp[i] % XP_MAX + XP_MAX * figure_xp_bonus(context, player, s, i);
  }
  store_stats(context, player, s, VALUES_ALL);
}

void AccumulateTechXP(BtechContext *context, DbRef pilot, MECH *mech,
                      int reason) {
  int xp;
  char *skname;
  static char *techw = "technician-weapons";

  if (mech) {
    if (!(skname = FindTechSkillName(mech)))
      return;
  } else
    skname = techw;

  xp = MAX(1, reason);

  // We emit all tech XP gains to the MechTechXP channel.
  if (char_gainxp(context, pilot, skname, xp))
    SendTechXP(context, tprintf("%s gained %d %s XP (changing mech #%ld)",
                                game_object_name(context->database, pilot), xp,
                                skname, mech ? mech->mynum : -1));
}

void AccumulateTechWeaponsXP(BtechContext *context, DbRef pilot, MECH *mech,
                             int reason) {
  char *skname;
  int xp;
  static char *techw = "technician-weapons";

  skname = techw;
  xp = MAX(1, reason);

  // We emit all tech xp gains to MechTechXP channel.
  if (char_gainxp(context, pilot, skname, xp))
    SendTechXP(context, tprintf("%s gained %d %s XP (changing mech #%ld)",
                                game_object_name(context->database, pilot), xp,
                                skname, mech ? mech->mynum : -1));
}

void AccumulateCommXP(DbRef pilot, MECH *mech) {
  BtechContext *context = mech->xcode.context;
  int xp;

  xp = 1;
  if (!RGotPilot(mech))
    return;
  if (!is_in_character(mech->xcode.context->database, mech->mynum))
    return;
  if (!is_connected(mech->xcode.context->database, pilot))
    return;
  if (char_gainxp(context, pilot, "Comm-Conventional", xp))
    SendXP(context,
           tprintf("%s gained %d %s XP (in #%ld)",
                   game_object_name(mech->xcode.context->database, pilot), xp,
                   "Comm-Conventional", mech->mynum));
}

void AccumulatePilXP(DbRef pilot, MECH *mech, int reason, int addanyway) {
  BtechContext *context = mech->xcode.context;
  char *skname;
  int xp;

  if (!is_in_character(mech->xcode.context->database, mech->mynum))
    return;

  if (!RGotPilot(mech))
    return;

  if (!(skname = FindPilotingSkillName(mech)))
    return;

  if (!addanyway) {
    if (MechLX(mech) != MechX(mech) || MechLY(mech) != MechY(mech)) {
      MechLX(mech) = MechX(mech);
      MechLY(mech) = MechY(mech);
    } else
      return;
  }
  xp = MAX(1, reason);

  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, skname, xp))
    SendPilotXP(context,
                tprintf("%s gained %d %s XP",
                        game_object_name(mech->xcode.context->database, pilot),
                        xp, skname));
  /*
      if (char_gainxp(context, pilot, skname, xp))
              SendXP(context, tprintf("%s gained %d %s XP",
     game_object_name(mech->xcode.context->database, pilot), xp,
     skname));
  */
}

void AccumulateSpotXP(DbRef pilot, MECH *attacker, MECH *wounded) {
  BtechContext *context = attacker->xcode.context;
  int xp = 1;

  if (!is_in_character(attacker->xcode.context->database, attacker->mynum))
    return;
  if (!RGotPilot(attacker))
    return;
  if (MechPilot(attacker) != pilot)
    return;
  if (attacker == wounded)
    return;
  if (Destroyed(wounded))
    return;
  if (MechTeam(wounded) == MechTeam(attacker))
    return;
  if (!is_in_character(attacker->xcode.context->database, wounded->mynum))
    return;
  if (char_gainxp(context, pilot, "Gunnery-Spotting", xp))
    SendXP(context,
           tprintf("%s gained spotting XP",
                   game_object_name(attacker->xcode.context->database, pilot)));
}

int MadePerceptionRoll(MECH *mech, int modifier) {
  BtechContext *context = mech->xcode.context;
  int pilot;

  if (!is_in_character(mech->xcode.context->database, mech->mynum))
    return 0;
  if (!RGotGPilot(mech))
    return 0;
  pilot = MechPilot(mech);
  if (pilot <= 0)
    return 0;
  if (!MechPer(mech))
    MechPer(mech) = char_getskilltarget(context, pilot, "Perception", 2);
  if (btech_random_roll(mech->xcode.context) < (MechPer(mech) + modifier))
    return 0;
  if (char_gainxp(context, pilot, "Perception", 1))
    SendXP(context,
           tprintf("%s gained 1 perception XP",
                   game_object_name(mech->xcode.context->database, pilot)));
  return 1;
}

void AccumulateArtyXP(DbRef pilot, MECH *attacker, MECH *wounded) {
  BtechContext *context = attacker->xcode.context;
  int xp = 1;

  /* If not in character ie: like in simulator - no xp */
  if (!is_in_character(attacker->xcode.context->database, attacker->mynum))
    return;

  if (!RGotGPilot(attacker))
    return;

  if (GunPilot(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed units */
  if (Destroyed(wounded))
    return;

  /* No xp if both on same team */
  if (MechTeam(wounded) == MechTeam(attacker))
    return;

  /* If target not in character ie: in simulator - no xp */
  if (!is_in_character(attacker->xcode.context->database, wounded->mynum))
    return;

  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, "Gunnery-Artillery", xp))
    SendAttackXP(context, tprintf("%s gained %d artillery XP",
                                  game_object_name(
                                      attacker->xcode.context->database, pilot),
                                  xp));
}

void AccumulateComputerXP(DbRef pilot, MECH *mech, int reason) {
  if (!mech)
    return;
  BtechContext *context = mech->xcode.context;

  if (mech && is_in_character(mech->xcode.context->database, mech->mynum) &&
      is_player(mech->xcode.context->database, pilot))
    if (char_gainxp(context, pilot, "computer", MAX(1, reason)))
      SendXP(context,
             tprintf("%s gained %d computer XP (mech #%ld)",
                     game_object_name(mech->xcode.context->database, pilot),
                     reason, mech ? mech->mynum : -1));
}

int HasBoolAdvantage(BtechContext *context, DbRef player, const char *name) {
  PSTATS stats, *s = &stats;
  char buf[SBUF_SIZE];

  strcpy(buf, name);
  retrieve_stats(context, player, VALUES_ATTRS | VALUES_ADVS | VALUES_HEALTH,
                 s);
  if (char_gvalue(s, buf) == 1)
    return 1;
  else
    return 0;
}

const int bth_modifier[] = /* Starts from '3' , in 1/36's */
    {
        /*  3 4 5  6  7  8  9 10 11 12 */
        1, 3, 6, 10, 15, 21, 26, 30, 33, 35, 0, 0, 0, 0 /* pad, just in case */
};

#define TonValue(mech)                                                         \
  MAX(1, (MechTons(mech) / ((MechType(mech) != CLASS_MECH) ? 2 : 1) /          \
          ((MechMove(mech) == MOVE_NONE) ? 2 : 1)))

static int t_mod(float sp) {
  if (sp <= MP2)
    return 0;
  if (sp <= MP4)
    return 1;
  if (sp <= MP6)
    return 2;
  if (sp <= MP9)
    return 3;
  return 4; /* No extra mods */
}

#define MoveValue(mech) (t_mod(MMaxSpeed(mech)) + 2)
#define NewMoveValue(mech) ((int)(MechMaxSpeed(mech) / MP1))

float getPilotBVMod(MECH *mech, int weapindx) {
  /*
   * What we do is we get the mod as if we had a 0+ piloting (baseline)
   * for the gun skill we want. Each '+' above zero subtracts .05 from
   * the result. Obviously, each '+' below adds .05.
   *
   * The first number in the array below corresponds to a 0+ 0+ person
   * and the last number in the array below corresponds to a 7+ 0+ person
   * (that's <gun skill>+ <pilot skill>+)
   */

  float zeroPilotBaseSkills[] = {2.05, 1.85, 1.65, 1.45, 1.25, 1.15, 1.05, .95};

  int myGSkill = FindPilotGunnery(mech, weapindx);
  int myPSkill = FindPilotPiloting(mech);
  float baseMod = 0.0;

  /* First we check if we have a totally off the wall GSkill, i.e., below
   * 0 or above 7.
   */
  if (myGSkill < 0) {
    baseMod = zeroPilotBaseSkills[0] + (abs(myGSkill) * 0.20);
  } else if (myGSkill > 7) {
    baseMod = zeroPilotBaseSkills[7] - (myGSkill * 0.10);
  } else {
    baseMod = zeroPilotBaseSkills[myGSkill];
  }

  return (baseMod - ((0 + myPSkill) * 0.05));
}

/*
 * Routines and formula for XP gain.
 */
void AccumulateGunXP(DbRef pilot, MECH *attacker, MECH *wounded, int damage,
                     float multiplier, int weapindx, int bth) {
  BtechContext *context = attacker->xcode.context;
  int xp, my_BV, th_BV, my_speed, th_speed;
  float myPilotBVMod = 1.0, theirPilotBVMod = 1.0;
  float weapTypeMod;
  char *skname;
  char buf[MBUF_SIZE];
  int damagemod;
  float vrtmod;
  int recycle_time;
  int weapon_battle_value;
  int i;
  int j = NUM_SECTIONS;

  weapTypeMod = 1;

  if (attacker->xcode.context->configuration->btech_oldxpsystem) {
    AccumulateGunXPold(pilot, attacker, wounded, damage, multiplier, weapindx,
                       bth);
    return;
  }

  /* No XP for zero'd mechas */
  for (i = 0; i < NUM_SECTIONS; i++)
    j -= SectIsDestroyed(wounded, i);

  if (j < 1)
    return;

  /* Is attacker in character ie: not in simulator */
  if (!is_in_character(attacker->xcode.context->database, attacker->mynum))
    return;

  if (NoGunXP(wounded)) /* No Gun XP for shooting this (Boxes, etc) */
    return;

  if (!RGotGPilot(attacker))
    return;

  if (GunPilot(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed mechs */
  if (Destroyed(wounded))
    return;

  /* No xp for shooting a teammate */
  if (MechTeam(wounded) == MechTeam(attacker))
    return;

  /* Is the target in character ie: in simulators */
  if (!is_in_character(attacker->xcode.context->database, wounded->mynum))
    return;

  /* No skill to match the weapon we're shooting with? */
  if (!(skname = FindGunnerySkillName(attacker, weapindx)))
    return;

  /* No xp for shooting mechwarriors if you not a mechwarrior */
  if (MechType(wounded) == CLASS_MW && MechType(attacker) != CLASS_MW)
    return;

  /* bth to high so no way to hit */
  if (!(bth <= 12))
    return;

  multiplier =
      multiplier * attacker->xcode.context->configuration->btech_xp_modifier;

  if (attacker->xcode.context->configuration->btech_xp_bthmod) {
    if (!(bth >= 3 && bth <= 12)) {
      if (attacker->xcode.context->configuration->btech_noisy_xpgain)
        SendXP(context, tprintf("#%ld in #%ld 1 noxp #%ld", pilot,
                                attacker->mynum, wounded->mynum));
      return; /* sure hits aren't interesting */
    }
    multiplier = 2 * multiplier * bth_modifier[bth - 3] / 36;
  }

  /* Need to do a BV mod between the mechs */
  my_BV = MechBV(attacker);
  th_BV = MechBV(wounded);

  if (attacker->xcode.context->configuration->btech_xp_usePilotBVMod) {
    myPilotBVMod = getPilotBVMod(attacker, weapindx);
    theirPilotBVMod = getPilotBVMod(wounded, weapindx);

    my_BV = my_BV * myPilotBVMod;
    th_BV = th_BV * theirPilotBVMod;

#ifdef XP_DEBUG
    SendDebug(context,
              tprintf("Using skill modified battle value for mechs %ld and %ld "
                      "with skill mods of %2.2f and %2.2f",
                      attacker->mynum, wounded->mynum, myPilotBVMod,
                      theirPilotBVMod));
#endif
  }

  my_speed = NewMoveValue(attacker) + 1;
  th_speed = NewMoveValue(wounded) + 1;

  if (MechWeapons[weapindx].type == TMISSILE)
    weapTypeMod = attacker->xcode.context->configuration->btech_xp_missilemod;
  else if (MechWeapons[weapindx].type == TAMMO)
    weapTypeMod = attacker->xcode.context->configuration->btech_xp_ammomod;

  if (attacker->xcode.context->configuration->btech_defaultweapdam > 1)
    damagemod = damage;
  else
    damagemod = 1;

  recycle_time =
      btech_weapon_settings_recycle_time(&context->weapon_settings, weapindx);
  weapon_battle_value =
      btech_weapon_settings_battle_value(&context->weapon_settings, weapindx);
  if (attacker->xcode.context->configuration->btech_xp_vrtmod)
    vrtmod = (recycle_time < 30 ? sqrt((double)recycle_time / 30.0) : 1);
  else
    vrtmod = 1.0;

  multiplier =
      (vrtmod * weapTypeMod * multiplier *
       sqrt((double)(th_BV + 1) * th_speed *
            attacker->xcode.context->configuration->btech_defaultweapbv /
            attacker->xcode.context->configuration->btech_defaultweapdam)) /
      (sqrt((double)(my_BV + 1) * my_speed * weapon_battle_value / damagemod));

  if (attacker->xcode.context->configuration->btech_perunit_xpmod)
    multiplier =
        multiplier *
        MechXPMod(attacker); /* Per unit XP Mod. Defaults to 1 anyways */

  /* Change the Cap to be variable depending on what a mux wants */

  xp = BOUNDED(1, (int)(multiplier * damage / 100),
               attacker->xcode.context->configuration->btech_xpgain_cap);

  strcpy(buf,
         game_object_name(attacker->xcode.context->database, wounded->mynum));

  // Emit XP gain over MechAttackXP
  if (char_gainxp(context, pilot, skname, (int)xp)) {
    SendAttackXP(
        context,
        tprintf("%s gained %d gun XP from feat of %f/100 difficulty "
                "(%d damage) against %s",
                game_object_name(attacker->xcode.context->database, pilot),
                (int)xp, multiplier, damage, buf));
    if (attacker->xcode.context->configuration->btech_noisy_xpgain)
      SendXP(context, tprintf("#%ld in #%ld %d damage #%ld", pilot,
                              attacker->mynum, damage, wounded->mynum));
  }

} // end AccumulateGunXP()

void AccumulateGunXPold(DbRef pilot, MECH *attacker, MECH *wounded,
                        int numOccurences, float multiplier, int weapindx,
                        int bth) {
  BtechContext *context = attacker->xcode.context;
  int xp;
  char *skname;
  char buf[MBUF_SIZE];

  /* Is the attacker in character ie: in simulators */
  if (!is_in_character(attacker->xcode.context->database, attacker->mynum))
    return;

  if (!RGotGPilot(attacker))
    return;

  if (GunPilot(attacker) != pilot)
    return;

  /* No xp for shooting yourself */
  if (attacker == wounded)
    return;

  /* No xp for shooting destroyed units */
  if (Destroyed(wounded))
    return;

  /* No xp for shooting teammate */
  if (MechTeam(wounded) == MechTeam(attacker))
    return;

  /* if target is in character ie: in simulators or something */
  if (!is_in_character(attacker->xcode.context->database, wounded->mynum))
    return;

  if (!(skname = FindGunnerySkillName(attacker, weapindx)))
    return;

  /* No xp for shooting a mechwarrior unless you a mechwarrior */
  if (MechType(wounded) == CLASS_MW && MechType(attacker) != CLASS_MW)
    return;

  if (!(bth >= 3 && bth <= 12))
    return; /* sure hits aren't interesting */

  if (MechTons(attacker) > 0)
    multiplier = multiplier *
                 BOUNDED(50, 100 * TonValue(wounded) / TonValue(attacker), 150);
  else {
    /* Bring this to the attention of the admins */
    SendError(
        context,
        tprintf("AccumulateGunXP: Weird tonnage for IC mech #%ld (%s): %d",
                attacker->mynum,
                game_object_name(attacker->xcode.context->database,
                                 attacker->mynum),
                (short)MechTons(attacker)));
    return;
  }

  /* Hmm.. we have to figure the speed differences as well */
  {
    int my_speed = MoveValue(attacker);
    int th_speed = MoveValue(wounded);

    multiplier = multiplier * th_speed * th_speed / my_speed / my_speed;
  }

  multiplier = multiplier * bth_modifier[bth - 3] / 36;
  multiplier = multiplier * 2; /* For average shot */
  if (attacker->xcode.context->configuration->btech_perunit_xpmod)
    multiplier = multiplier *
                 MechXPMod(attacker); /* Per unit XP Modifier. Defaults to 1 */

  if (btech_random_range(attacker->xcode.context, 1, 50) >
      (multiplier * numOccurences))
    return; /* Nothing for truly twinky stuff, occasionally */

  xp = BOUNDED(1, (int)(multiplier * numOccurences) / 100,
               50); /*Hardcoded limit */
  strcpy(buf,
         game_object_name(attacker->xcode.context->database, wounded->mynum));
  /* Switching to Exile method of tracking xp, where we split
   * Attacking and Piloting xp into two different channels
   */
  if (char_gainxp(context, pilot, skname, (int)xp))
    SendAttackXP(context, tprintf("%s gained %d gun XP from feat of %f %% "
                                  "difficulty (%d occurences) against %s",
                                  game_object_name(
                                      attacker->xcode.context->database, pilot),
                                  (int)xp, multiplier, numOccurences, buf));
}

void fun_btgetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  PSTATS stats;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = flaggo (?) */
  DbRef target;
  int targetcode, flaggo;

  FUNCHECK((target = char_lookupplayer(context, player, cause, 0, fargs[0])) ==
               NOTHING,
           "#-1 INVALID TARGET");
  FUNCHECK(!Wiz(context->database, player), "#-1 PERMISSION DENIED!");
  if (Readnum(targetcode, fargs[1]))
    targetcode = char_getvaluecode(context, fargs[1]);
  FUNCHECK(targetcode < 0 || targetcode >= (int)(NUM_CHARVALUES),
           "#-1 INVALID VALUE");
  flaggo = atoi(fargs[2]);
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 4) {
    safe_tprintf_str(buff, bufc, "%d",
                     figure_xp_to_next_level(context, target, targetcode));
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 3) {
    retrieve_stats(context, target, VALUES_SKILLS, &stats);
    safe_tprintf_str(buff, bufc, "%d", stats.values[targetcode]);
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo == 2) {
    safe_tprintf_str(buff, bufc, "%d",
                     char_getxpbycode(context, target, targetcode));
    return;
  }
  if (char_values[targetcode].type == CHAR_SKILL && flaggo) {
    safe_tprintf_str(buff, bufc, "%d",
                     char_getskilltargetbycode(context, target, targetcode, 0));
    return;
  }
  safe_tprintf_str(buff, bufc, "%d",
                   char_getvaluebycode(context, target, targetcode));
}

void fun_btsetcharvalue(char *buff, char **bufc, DbRef player, DbRef cause,
                        char *fargs[], int nfargs, char *cargs[], int ncargs,
                        EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  /* fargs[0] = char id (#222)
     fargs[1] = value name / value loc #
     fargs[2] = value to be set
     fargs[3] = flaggo (?)
   */
  DbRef target;
  int targetcode, targetvalue, flaggo;

  FUNCHECK((target = char_lookupplayer(context, player, cause, 0, fargs[0])) ==
               NOTHING,
           "#-1 INVALID TARGET");
  FUNCHECK(!Wiz(context->database, player), "#-1 PERMISSION DENIED!");
  if (Readnum(targetcode, fargs[1]))
    targetcode = char_getvaluecode(context, fargs[1]);
  FUNCHECK(targetcode < 0 || targetcode >= (int)(NUM_CHARVALUES),
           "#-1 INVALID VALUE");
  targetvalue = atoi(fargs[2]);
  flaggo = atoi(fargs[3]);

  /* We supposedly have everything at hand.. */
  if (flaggo) {
    FUNCHECK(char_values[targetcode].type != CHAR_SKILL,
             "#-1 ONLY SKILLS CAN HAVE FLAG");
  }
  switch (flaggo) {
  case 0:
    /* this is the # of skill points in said skill
     * Also Known as Level. This is not the + value
     * I.e. Setting someone to Level 2 Gun-Bmech with A Physical Attribute of 7+
     * will give you a 5+ in Gun-Bmech */
    char_setvaluebycode(context, target, targetcode, targetvalue);
    safe_tprintf_str(buff, bufc, "%s's %s set to %d",
                     game_object_name(context->database, target),
                     char_values[targetcode].name,
                     char_getvaluebycode(context, target, targetcode));
    break;

  case 1:
    /* This is the + value of said skill
     * Also known as the ToHit Roll. This is not the 'Skill Level'
     * I.e. Setting someone's Gun-Bmech with this to 5 with a Physical Attribute
     * of 7+ will give you Level 2 Gun-Bmech (5+) */

    char_setvaluebycode(context, target, targetcode, 0);
    targetvalue =
        char_getskilltargetbycode(context, target, targetcode, 0) - targetvalue;

    /* Handle a wierd code race issue. target shouldn't be negative in this case
     * anyways */
    if (targetvalue >= 0) {
      char_setvaluebycode(context, target, targetcode, targetvalue);
    } else {
      char_setvaluebycode(context, target, targetcode, 0);
    }

    safe_tprintf_str(buff, bufc, "%s's %s set to %d",
                     game_object_name(context->database, target),
                     char_values[targetcode].name,
                     targetvalue >= 0
                         ? char_getvaluebycode(context, target, targetcode)
                         : 0);

    break;

  case 3:
    /* Set the XP Amount for this skill */
    char_gainxpbycode(
        context, target, targetcode,
        targetvalue - char_getxpbycode(context, target, targetcode), 1);

    SendXP(context, tprintf("%ld set %ld's %s XP to %d", player, target,
                            char_values[targetcode].name, targetvalue));
    safe_tprintf_str(buff, bufc, "%s's %s XP set to %d.",
                     game_object_name(context->database, target),
                     char_values[targetcode].name, targetvalue);

    break;

  default:
    /* Any other flaggo value will addxp for the skill */
    char_gainxpbycode(context, target, targetcode, targetvalue, 1);
    SendXP(context, tprintf("#%ld added %d more %s XP to #%ld", player,
                            targetvalue, char_values[targetcode].name, target));
    safe_tprintf_str(buff, bufc, "%s gained %d more %s XP.",
                     game_object_name(context->database, target), targetvalue,
                     char_values[targetcode].name);

    break;
  }
}

/* ----------------------------------------------------------------------
** Syntax: btcharlist(skills|advantages|attributes[,targetplayer])
**
** Given one of the three arguments above, btcharlist returns the
** listing of each in a space delimited list.  This is basically a
** function version of +show. If the second argument is provided, only
** the skills/advantages that are learned or possessed will
** appear. For attributes the full list will be returned of since
** characters need all of them.
*/
void fun_btcharlist(char *buff, char **bufc, DbRef player, DbRef cause,
                    char *fargs[], int nfargs, char *cargs[], int ncargs,
                    EvaluationContext *evaluation) {
  BtechContext *context = evaluation->btech;
  int i;
  int type = 0;
  int first = 1;
  DbRef target = 0;
  enum {
    CHSKI,
    CHADV,
    CHATT,
  };
  static char *cmds[] = {"skills", "advantages", "attributes", NULL};

  if (!argument_count_in_range("BTCHARLIST", nfargs, 1, 2, buff, bufc))
    return;

  if (nfargs == 2) {
    target = char_lookupplayer(context, player, cause, 0, fargs[1]);
    if (target == NOTHING) {
      safe_str("#-1 FUNCTION (BTCHARLIST) INVALID TARGET", buff, bufc);
      return;
    }
  }

  switch (listmatch(cmds, fargs[0])) {
  case CHSKI:
    type = CHAR_SKILL;
    break;
  case CHADV:
    type = CHAR_ADVANTAGE;
    break;
  case CHATT:
    type = CHAR_ATTRIBUTE;
    break;
  default:
    safe_str("#-1 FUNCTION (BTCHARLIST) INVALID VALUE", buff, bufc);
    return;
  }

  for (i = 0; i < (int)(NUM_CHARVALUES); ++i)
    if (type == char_values[i].type) {
      if (nfargs == 2 && type != CHAR_ATTRIBUTE) {
        int targetcode = char_getvaluecode(context, char_values[i].name);
        if (char_getvaluebycode(context, target, targetcode) == 0 &&
            (type == CHAR_SKILL &&
             char_getxpbycode(context, target, targetcode) == 0))
          continue;
      }
      if (first)
        first = 0;
      else
        safe_str(" ", buff, bufc);
      safe_str(char_values[i].name, buff, bufc);
    }
  return;
}

#define MAX_PLAYERS_ON 10000

void debug_xptop(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  BtechContext *context = debug->context;
  int hm, i, j;
  DbRef top[MAX_PLAYERS_ON];
  int topv[MAX_PLAYERS_ON];
  int count = 0, gt = 0;
  coolmenu *c = NULL;
  PSTATS stats, *s = &stats;

  bzero(top, sizeof(top));
  bzero(topv, sizeof(topv));
  skipws(buffer);
  DOCHECK_CONTEXT(context, !*buffer, "Invalid argument!");
  DOCHECK_CONTEXT(context, (hm = char_getvaluecode(context, buffer)) < 0,
                  "Invalid value name!");
  DOCHECK_CONTEXT(context, char_values[hm].type != CHAR_SKILL,
                  "Only skills have XP (for now at least)");
  DO_WHOLE_DB(context->database, i) {
    if (!is_player(context->database, i))
      continue;
    if (Wiz(context->database, i))
      continue;
    retrieve_stats(context, i, VALUES_SKILLS, s);
    if (!s->xp[hm])
      continue;
    top[count] = i;
    topv[count] = s->xp[hm] % XP_MAX;
    gt += topv[count];
    count++;
  }
  for (i = 0; i < (count - 1); i++)
    for (j = i + 1; j < count; j++) {
      if (topv[j] > topv[i]) {
        topv[count] = topv[j];
        topv[j] = topv[i];
        topv[i] = topv[count];

        top[count] = top[j];
        top[j] = top[i];
        top[i] = top[count];
      }
    }
  addline();
  for (i = 0; i < MIN(16, count); i++) {
    addmenu(
        tprintf("%3d. %s", i + 1, game_object_name(context->database, top[i])));
    addmenu(tprintf("%d (%.3f %%)", topv[i], (100.0 * topv[i]) / gt));
  }
  addline();
  if (gt) {
    addmenu(tprintf("Grand total: %d points", gt));
    addline();
  }
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

static void store_health(BtechContext *context, DbRef player, PSTATS *s) {
  silly_atr_set_in(
      context->database, player, A_HEALTH,
      tprintf("%d,%d", char_gvalue(s, "Bruise"), char_gvalue(s, "Lethal")));
}

static void retrieve_health(BtechContext *context, DbRef player, PSTATS *s) {
  char *c = btech_attribute_read(context->database, player, A_HEALTH,
                                 (char[LBUF_SIZE]){0});
  PSTATS *s1;
  int i1, i2;

  if (sscanf(c, "%d,%d", &i1, &i2) != 2) {
    s1 = create_new_stats();
    memcpy(s, s1, sizeof(PSTATS));
    store_stats(context, player, s, VALUES_ALL);
    free((void *)s1);
    return;
  }
  char_svalue(s, "Bruise", i1);
  char_svalue(s, "Lethal", i2);
}

static void store_attrs(BtechContext *context, DbRef player, PSTATS *s) {
  silly_atr_set_in(context->database, player, A_ATTRS,
                   tprintf("%d,%d,%d,%d,%d", char_gvalue(s, "Build"),
                           char_gvalue(s, "Reflexes"),
                           char_gvalue(s, "Intuition"), char_gvalue(s, "Learn"),
                           char_gvalue(s, "Charisma")));
}

static void retrieve_attrs(BtechContext *context, DbRef player, PSTATS *s) {
  char *c = btech_attribute_read(context->database, player, A_ATTRS,
                                 (char[LBUF_SIZE]){0});
  PSTATS *s1;
  int i1, i2, i3, i4, i5;

  if (sscanf(c, "%d,%d,%d,%d,%d", &i1, &i2, &i3, &i4, &i5) != 5) {
    s1 = create_new_stats();
    memcpy(s, s1, sizeof(PSTATS));
    store_stats(context, player, s, VALUES_ALL);
    free((void *)s1);
    return;
  }
  char_svalue(s, "Build", i1);
  char_svalue(s, "Reflexes", i2);
  char_svalue(s, "Intuition", i3);
  char_svalue(s, "Learn", i4);
  char_svalue(s, "Charisma", i5);
}

static void generic_retrieve_stuff(BtechContext *context, DbRef player,
                                   PSTATS *s, int attrnum) {
  char *c = btech_attribute_read(context->database, player, attrnum,
                                 (char[LBUF_SIZE]){0}),
       *e;
  char buf[512];
  int i1, i2, i3, sn;

  if (!*c)
    return;
  while (1) {
    i2 = i3 = 0;
    e = strchr(c, '/');
    if (sscanf(c, "%[A-Za-z_-]:%d,%d,%d", buf, &i1, &i2, &i3) < 2)
      return;
    /* Do the magic ;) */
    sn = char_getvaluecode(context, buf);
    if (sn >= 0) {
      s->values[sn] = i1;
      if (i2)
        s->xp[sn] = i2;
      if (i3)
        s->last_use[sn] = i3;
    }
    if (!(c = e))
      return;
    c++;
    if (!(*c))
      return;
  }
}

static void generic_store_stuff(BtechContext *context, DbRef player, PSTATS *s,
                                int attrnum, int flag) {
  char buf[LBUF_SIZE] = {0};
  int i;
  char *c;

  c = buf;
  for (i = 0; i < (int)(NUM_CHARVALUES); i++) {
    if (!s->values[i] && !s->xp[i])
      continue;
    if (flag) {
      if (char_values[i].type != CHAR_SKILL)
        continue;
    } else if (i != 5 && char_values[i].type != CHAR_ADVANTAGE)
      continue;
    if (s->xp[i])
      snprintf(c, buf - c, "%s:%d,%d,%d/", context->char_value_short_names[i],
               s->values[i], s->xp[i], (int)s->last_use[i]);
    else
      snprintf(c, buf - c, "%s:%d/", context->char_value_short_names[i],
               s->values[i]);
    while (*(++c))
      ;
  }
  if (*buf)
    silly_atr_set_in(context->database, player, attrnum, buf);
  else
    silly_atr_set_in(context->database, player, attrnum, "");
}

static void retrieve_skills(BtechContext *context, DbRef player, PSTATS *s) {
  generic_retrieve_stuff(context, player, s, A_SKILLS);
}

static void retrieve_advs(BtechContext *context, DbRef player, PSTATS *s) {
  generic_retrieve_stuff(context, player, s, A_ADVS);
}

static void store_skills(BtechContext *context, DbRef player, PSTATS *s) {
  generic_store_stuff(context, player, s, A_SKILLS, 1);
}

static void store_advs(BtechContext *context, DbRef player, PSTATS *s) {
  generic_store_stuff(context, player, s, A_ADVS, 0);
}

static void store_stats(BtechContext *context, DbRef player, PSTATS *s,
                        int modes) {
  if (!is_player(context->database, player))
    return;
  if (modes & VALUES_HEALTH)
    store_health(context, player, s);
  if (modes & VALUES_ATTRS)
    store_attrs(context, player, s);
  if (modes & VALUES_ADVS) {
    if (player == context->cached_target_character)
      context->cached_target_character = -1;
    store_advs(context, player, s);
  }
  if (modes & VALUES_SKILLS) {
    if (player == context->cached_target_character)
      context->cached_target_character = -1;
    store_skills(context, player, s);
  }
}

static void retrieve_stats(BtechContext *context, DbRef player, int modes,
                           PSTATS *stats) {
  bzero(stats, sizeof(*stats));
  if (modes & VALUES_HEALTH)
    retrieve_health(context, player, stats);
  if (modes & VALUES_ADVS)
    retrieve_advs(context, player, stats);
  if (modes & VALUES_ATTRS)
    retrieve_attrs(context, player, stats);
  if (modes & VALUES_SKILLS)
    retrieve_skills(context, player, stats);
}

void debug_setxplevel(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  BtechContext *context = debug->context;
  char *args[3];
  int xpt, code;

  DOCHECK_CONTEXT(context, mech_parseattributes(buffer, args, 3) != 2,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(context, Readnum(xpt, args[1]), "Invalid value!");
  DOCHECK_CONTEXT(context, xpt < 0,
                  "Threshold needs to be >=0 (0 = no gains possible)");
  DOCHECK_CONTEXT(context, (code = char_getvaluecode(context, args[0])) < 0,
                  "That isn't any charvalue!");
  DOCHECK_CONTEXT(context, char_values[code].type != CHAR_SKILL,
                  "That isn't any skill!");
  char_values[code].xpthreshold = xpt;
  log_error(context->log, LOG_WIZARD, "WIZ", "CHANGE",
            "Exp threshold for %s changed to %d by #%ld",
            char_values[code].name, xpt, player);
}

int btthreshold_func(BtechContext *context, char *skillname) {
  int code;

  if (!skillname || !*skillname)
    return -1;
  code = char_getvaluecode(context, skillname);
  if (code < 0)
    return -1;
  if (char_values[code].type != CHAR_SKILL)
    return -1;
  return char_values[code].xpthreshold;
}
