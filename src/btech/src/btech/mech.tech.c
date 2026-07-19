/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech.h"
#include "mech.events.h"
#include "mech.tech.h"
#include "mux/network/mux_event.h"
#include "p.btechstats.h"
#include "p.mech.build.h"
#include "p.mech.partnames.h"
#include "p.mech.utils.h"

int game_lag(BtechContext *context) {
  if (!context->events->tick)
    return 0;
  return 100 * (context->clock->now - context->process_start_time) /
             context->events->tick -
         100;
}

int game_lag_time(BtechContext *context, int duration) {
  return (100 + game_lag(context)) * duration / 100;
}

int player_techtime(BtechContext *context, DbRef player) {
  /* Returns tech time, in minutes, for given player */

  time_t techtime;
  char *tt_attr;
  int tused;

  tt_attr = btech_attribute_read(context->database, player, A_TECHTIME,
                                 (char[LBUF_SIZE]){0});

  if (tt_attr) {
    techtime = (time_t)atoi(tt_attr);
    if (techtime < context->clock->now)
      techtime = context->clock->now;
  } else {
    techtime = context->clock->now;
  }

  tused = (techtime - context->clock->now) / TECH_TICK;

  return tused;
}

int tech_roll(DbRef player, MECH *mech, int diff) {
  BtechContext *context = mech->xcode.context;
  int s;
  int succ;
  int r = (HasBoolAdvantage(context, player, "tech_aptitude")
               ? char_rollsaving(mech->xcode.context)
               : btech_random_roll(mech->xcode.context));

  s = FindTechSkill(player, mech);
  s += diff;
  succ = r >= s;
  if (is_wizard(context->database, player)) {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Tech - BTH: %d(Base:%d, Mod:%d) Roll: %d", s, s - diff, diff,
                  r);
  } else {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "BTH: %d Roll: %d", s, r);
  }
  if (succ && is_in_character(context->database, mech->mynum))
    AccumulateTechXP(context, player, mech,
                     BOUNDED(1, s - 7, MAX(2, 1 + diff)));
  return (r - s);
}

int tech_weapon_roll(DbRef player, MECH *mech, int diff) {
  BtechContext *context = mech->xcode.context;
  int s;
  int succ;
  int r = (HasBoolAdvantage(context, player, "tech_aptitude")
               ? char_rollsaving(mech->xcode.context)
               : btech_random_roll(mech->xcode.context));

  s = char_getskilltarget(context, player, "technician-weapons", 0);
  s += diff;
  succ = r >= s;
  if (is_wizard(context->database, player)) {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Tech-W - BTH: %d(Base:%d, Mod:%d) Roll: %d", s, s - diff,
                  diff, r);
  } else {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "BTH: %d Roll: %d", s, r);
  }
  if (succ && is_in_character(context->database, mech->mynum))
    AccumulateTechWeaponsXP(context, player, mech,
                            BOUNDED(1, s - 7, MAX(2, 1 + diff)));
  return (r - s);
}

/* Basic idea: Check for attribute, if not set, set it, and do interesting
   stuff */

void tech_status(BtechContext *context, DbRef player, time_t dat) {
  char buf[MBUF_SIZE] = {0};
  char *olds;
  int un;

  if (dat <= 0) {
    olds = btech_attribute_read(context->database, player, A_TECHTIME,
                                (char[LBUF_SIZE]){0});
    if (olds) {
      dat = (time_t)atoi(olds);
      if (dat < context->clock->now)
        dat = context->clock->now;
    } else
      dat = context->clock->now;
  }
  if (dat <= context->clock->now)
    notify(btech_context_evaluation(context), player,
           "You have no jobs pending!");
  else {
    un = (dat - context->clock->now) / TECH_TICK;
    snprintf(buf, sizeof(buf), "You have %d %s%s of repairs pending", un,
             TECH_UNIT, un != 1 ? "s" : "");
    if (un >= context->configuration->btech_maxtechtime)
      snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf),
               " and you're too tired to do more efficiently.");
    else {
      un = context->configuration->btech_maxtechtime - un;
      snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf),
               " and you're ready to do at least %d more %s%s of work.", un,
               TECH_UNIT, un == 1 ? "" : "s");
    }
    notify(btech_context_evaluation(context), player, buf);
  }
}

int tech_addtechtime(BtechContext *context, DbRef player, int time) {
  time_t old;
  char *olds = btech_attribute_read(context->database, player, A_TECHTIME,
                                    (char[LBUF_SIZE]){0});

  if (olds) {
    old = (time_t)atoi(olds);
    if (old < context->clock->now)
      old = context->clock->now;
  } else
    old = context->clock->now;
  old += time * TECH_TICK;
  silly_atr_set_in(context->database, player, A_TECHTIME, tprintf("%ld", old));
  tech_status(context, player, old);
  return (old - context->clock->now);
}

int tech_parsepart_advanced(MECH *mech, char *buffer, int *loc, int *pos,
                            int *extra, int allowrear) {
  char *args[5];
  int l, argc, isrear = 0;

  if (!(argc = mech_parseattributes(buffer, args, 4)))
    return -1;
  if (argc > (2 + (extra != NULL)))
    return -1;
  if (!allowrear) {
    if ((!extra && argc != (1 + (pos != NULL))) ||
        (extra && (argc < (1 + (pos != NULL)) || argc > (2 + (pos != NULL)))))
      return -1;
  } else {
    if (argc == 2) {
      if (toupper(args[1][0]) != 'R')
        return -1;
      isrear = 8;
    }
  }
  if ((*loc = ArmorSectionFromString(MechType(mech), MechMove(mech), args[0])) <
      0)
    return -1;
  if (allowrear)
    *loc += isrear;
  if (pos) {
    l = atoi(args[1]) - 1;
    if (l < 0 || l >= CritsInLoc(mech, *loc))
      return -2;
    *pos = l;
  }
  if (extra) {
    if (argc > 2)
      *extra = args[2][0];
    else
      *extra = 0;
  }
  return 0;
}

int tech_parsepart(MECH *mech, char *buffer, int *loc, int *pos, int *extra) {
  return tech_parsepart_advanced(mech, buffer, loc, pos, extra, 0);
}

int tech_parsegun(MECH *mech, char *buffer, int *loc, int *pos, int *brand) {
  char *args[3];
  int l, argc, t, c = 0, pi, pb;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc < 1 || argc > (2 + (brand != NULL)))
    return -1;
  if (argc == (2 + (brand != NULL)) || (brand && argc == 2 && atoi(args[1]))) {
    if ((*loc = ArmorSectionFromString(MechType(mech), MechMove(mech),
                                       args[0])) < 0)
      return -1;
    l = atoi(args[1]);
    if (l <= 0 || l > CritsInLoc(mech, *loc))
      return -4;
    *pos = l - 1;
  } else {
    /* Check if it's a number */
    if (args[0][0] < '0' || args[0][0] > '9')
      return -1;
    l = atoi(args[0]);
    if (l < 0)
      return -1;
    if ((t = FindWeaponNumberOnMech(mech, l, loc, pos)) == -1)
      return -1;
  }
  t = GetPartType(mech, *loc, *pos);
  if (brand != NULL && argc > 1 && !atoi(args[argc - 1])) {
    if (!find_matching_long_part(mech->xcode.context, args[argc - 1], &c, &pi,
                                 &pb))
      return -2;
    if (pi != t)
      return -3;
    *brand = pb;
  } else if (brand != NULL)
    *brand = GetPartBrand(mech, *loc, *pos);
  return 0;
}

typedef struct LatestTechEventContext LatestTechEventContext;
struct LatestTechEventContext {
  int latest;
};

static void find_latest_tech_event(MuxEvent *event, void *data) {
  LatestTechEventContext *context = data;
  int offset = event->tick - event->scheduler->tick;
  long amount = (((long)event->data2) % PLAYERPOS) / 16 - 1;

  switch (event->type) {
  case EVENT_REPAIR_FIXI:
    offset += amount * FIXINTERNAL_TIME * TECH_TICK;
    break;
  case EVENT_REPAIR_FIX:
    offset += amount * FIXARMOR_TIME * TECH_TICK;
    break;
  }
  if (offset > context->latest)
    context->latest = offset;
}

int figure_latest_tech_event(MECH *mech) {
  LatestTechEventContext latest = {0};
  MuxEventScheduler *events = mech->xcode.context->events;

  for (int type = FIRST_TECH_EVENT; type <= LAST_TECH_EVENT; type++)
    mux_event_visit_type_data(events, type, mech, find_latest_tech_event,
                              &latest);
  return latest.latest;
}
