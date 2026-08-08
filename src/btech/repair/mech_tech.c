#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_obj_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_partnames_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "repair_job.h"

int tech_proper_armor_part(const Mech *mech) {
#ifdef BT_COMPLEXREPAIRS
  return ProperArmor(mech);
#else
  int technology = mech_technology_flags(mech);
  int secondary = mech_technology_flags_secondary(mech);
  int infantry = mech_infantry_technology_flags(mech);
  int armor = technology & FF_TECH                  ? FF_ARMOR
              : technology & HARDA_TECH             ? HD_ARMOR
              : secondary & STEALTH_ARMOR_TECH      ? STH_ARMOR
              : secondary & HVY_FF_ARMOR_TECH       ? HVY_FF_ARMOR
              : secondary & LT_FF_ARMOR_TECH        ? LT_FF_ARMOR
              : infantry & CS_PURIFIER_STEALTH_TECH ? PURIFIER_ARMOR
                                                    : S_ARMOR;
  return cargo_equipment_index(armor);
#endif
}

int tech_proper_internal_part(const Mech *mech) {
#ifdef BT_COMPLEXREPAIRS
  return ProperInternal(mech);
#else
  int technology = mech_technology_flags(mech);
  int internal = technology & ES_TECH       ? ES_INTERNAL
                 : technology & REINFI_TECH ? RE_INTERNAL
                 : technology & COMPI_TECH  ? CO_INTERNAL
                                            : S_INTERNAL;
  return cargo_equipment_index(internal);
#endif
}

int game_lag(BtechContext *context) {
  if (!context->events->tick)
    return 0;
  time_t const elapsed = context->clock->now - context->process_start_time;
  return clamp_intptr_to_int(100 * (intptr_t)elapsed / context->events->tick -
                             100);
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

  tused = clamp_intptr_to_int(
      (intptr_t)((techtime - context->clock->now) / TECH_TICK));

  return tused;
}

int tech_roll(DbRef player, Mech *mech, int diff) {
  BtechContext *context = mech_context(mech);
  int s;
  int succ;
  int r = (HasBoolAdvantage(context, player, "tech_aptitude")
               ? char_rollsaving(context)
               : btech_random_roll(context));

  s = FindTechSkill(player, mech);
  s += diff;
  succ = r >= s;
  if (is_wizard(btech_context_database(context), player)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Tech - BTH: %d(Base:%d, Mod:%d) Roll: %d", s, s - diff, diff,
                  r);
  } else {
    notify_printf(btech_context_evaluation(context), player, "BTH: %d Roll: %d",
                  s, r);
  }
  if (succ &&
      is_in_character(btech_context_database(context), mech_dbref(mech)))
    AccumulateTechXP(context, player, mech,
                     BOUNDED(1, s - 7, MAX(2, 1 + diff)));
  return (r - s);
}

int tech_weapon_roll(DbRef player, Mech *mech, int diff) {
  BtechContext *context = mech_context(mech);
  int s;
  int succ;
  int r = (HasBoolAdvantage(context, player, "tech_aptitude")
               ? char_rollsaving(context)
               : btech_random_roll(context));

  s = char_getskilltarget(context, player, "technician-weapons", 0);
  s += diff;
  succ = r >= s;
  if (is_wizard(btech_context_database(context), player)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Tech-W - BTH: %d(Base:%d, Mod:%d) Roll: %d", s, s - diff,
                  diff, r);
  } else {
    notify_printf(btech_context_evaluation(context), player, "BTH: %d Roll: %d",
                  s, r);
  }
  if (succ &&
      is_in_character(btech_context_database(context), mech_dbref(mech)))
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
    mecha_notify(btech_context_evaluation(context), player,
                 "You have no jobs pending!");
  else {
    un = clamp_intptr_to_int(
        (intptr_t)((dat - context->clock->now) / TECH_TICK));
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
    mecha_notify(btech_context_evaluation(context), player, buf);
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
  return clamp_intptr_to_int((intptr_t)(old - context->clock->now));
}

int tech_parsepart_advanced(Mech *mech, char *buffer, int *loc, int *pos,
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
  if ((*loc = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                     args[0])) < 0)
    return -1;
  if (allowrear)
    *loc += isrear;
  if (pos) {
    l = atoi(args[1]) - 1;
    if (l < 0 || l >= mech_section_critical_count(mech, *loc))
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

int tech_parsepart(Mech *mech, char *buffer, int *loc, int *pos, int *extra) {
  return tech_parsepart_advanced(mech, buffer, loc, pos, extra, 0);
}

int tech_parsegun(Mech *mech, char *buffer, int *loc, int *pos, int *brand) {
  char *args[3];
  int l, argc, t, c = 0, pi, pb;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc < 1 || argc > (2 + (brand != NULL)))
    return -1;
  if (argc == (2 + (brand != NULL)) || (brand && argc == 2 && atoi(args[1]))) {
    if ((*loc = ArmorSectionFromString(mech_class(mech),
                                       mech_movement_type(mech), args[0])) < 0)
      return -1;
    l = atoi(args[1]);
    if (l <= 0 || l > mech_section_critical_count(mech, *loc))
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
  t = mech_critical_part_type(mech, *loc, *pos);
  if (brand != NULL && argc > 1 && !atoi(args[argc - 1])) {
    if (!find_matching_long_part(mech_context(mech), args[argc - 1], &c, &pi,
                                 &pb))
      return -2;
    if (pi != t)
      return -3;
    *brand = pb;
  } else if (brand != NULL)
    *brand = mech_critical_brand(mech, *loc, *pos);
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

int figure_latest_tech_event(Mech *mech) {
  LatestTechEventContext latest = {0};
  for (int type = FIRST_TECH_EVENT; type <= LAST_TECH_EVENT; type++)
    mech_event_visit(mech, type, find_latest_tech_event, &latest);
  return latest.latest;
}
