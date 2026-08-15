#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* Implements BattleTech repair mechanics for unit tech. */

#include <limits.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
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
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "repair_job.h"

int tech_proper_armor_part(const Mech *mech) {
  int technology = mech_technology_flags(mech);
  int secondary = mech_technology_flags_secondary(mech);
  int infantry = mech_infantry_technology_flags(mech);
  int armor = S_ARMOR;
  if (technology & FF_TECH)
    armor = FF_ARMOR;
  else if (technology & HARDA_TECH)
    armor = HD_ARMOR;
  else if (secondary & STEALTH_ARMOR_TECH)
    armor = STH_ARMOR;
  else if (secondary & HVY_FF_ARMOR_TECH)
    armor = HVY_FF_ARMOR;
  else if (secondary & LT_FF_ARMOR_TECH)
    armor = LT_FF_ARMOR;
  else if (infantry & CS_PURIFIER_STEALTH_TECH)
    armor = PURIFIER_ARMOR;
  return cargo_equipment_index(armor);
}

int tech_proper_internal_part(const Mech *mech) {
  int technology = mech_technology_flags(mech);
  int internal = S_INTERNAL;
  if (technology & ES_TECH)
    internal = ES_INTERNAL;
  else if (technology & REINFI_TECH)
    internal = RE_INTERNAL;
  else if (technology & COMPI_TECH)
    internal = CO_INTERNAL;
  return cargo_equipment_index(internal);
}

int game_lag(BtechContext *context) {
  if (!context->events->tick)
    return 0;
  time_t const ELAPSED = context->clock->now - context->process_start_time;
  return clamp_intptr_to_int((100 * (intptr_t)ELAPSED / context->events->tick) -
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
    if (!parse_time_checked(tt_attr, &techtime))
      techtime = context->clock->now;
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
  int r = (has_bool_advantage(context, player, "tech_aptitude")
               ? char_rollsaving(context)
               : btech_random_roll(context));

  s = find_tech_skill(player, mech);
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
    accumulate_tech_xp(context, player, mech,
                       bounded(1, s - 7, max(2, 1 + diff)));
  return (r - s);
}

int tech_weapon_roll(DbRef player, Mech *mech, int diff) {
  BtechContext *context = mech_context(mech);
  int s;
  int succ;
  int r = (has_bool_advantage(context, player, "tech_aptitude")
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
    accumulate_tech_weapons_xp(context, player, mech,
                               bounded(1, s - 7, max(2, 1 + diff)));
  return (r - s);
}

/* Basic idea: Check for attribute, if not set, set it, and do interesting
   stuff */

typedef struct TechStatusRequest {
  BtechContext *context;
  DbRef player;
  time_t completion;
} TechStatusRequest;

static void tech_status(const TechStatusRequest *request) {
  BtechContext *context = request->context;
  const DbRef PLAYER = request->player;
  time_t dat = request->completion;
  char buf[MBUF_SIZE] = {0};
  char *olds;
  int un;

  if (dat <= 0) {
    olds = btech_attribute_read(context->database, PLAYER, A_TECHTIME,
                                (char[LBUF_SIZE]){0});
    if (olds) {
      if (!parse_time_checked(olds, &dat))
        dat = context->clock->now;
      if (dat < context->clock->now)
        dat = context->clock->now;
    } else {
      dat = context->clock->now;
    }
  }
  if (dat <= context->clock->now) {
    mecha_notify(btech_context_evaluation(context), PLAYER,
                 "You have no jobs pending!");
  } else {
    un = clamp_intptr_to_int(
        (intptr_t)((dat - context->clock->now) / TECH_TICK));
    (void)snprintf(buf, sizeof(buf), "You have %d %s%s of repairs pending", un,
                   TECH_UNIT, un != 1 ? "s" : "");
    size_t used = strlen(buf);
    char *append_at = checked_storage_at(buf, sizeof(buf), sizeof(*buf), used);
    if (un >= context->configuration->btech_maxtechtime) {
      (void)snprintf(append_at, sizeof(buf) - used,
                     " and you're too tired to do more efficiently.");
    } else {
      un = context->configuration->btech_maxtechtime - un;
      (void)snprintf(append_at, sizeof(buf) - used,
                     " and you're ready to do at least %d more %s%s of work.",
                     un, TECH_UNIT, un == 1 ? "" : "s");
    }
    mecha_notify(btech_context_evaluation(context), PLAYER, buf);
  }
}

int tech_addtechtime(const TechTimeAddition *addition) {
  char message_buffer[128];
  BtechContext *context = addition->context;
  const DbRef PLAYER = addition->player;
  int added_seconds = tech_time_scaled_seconds(context, addition->units);
  if (added_seconds == 0)
    return 1;

  time_t old;
  char *olds = btech_attribute_read(context->database, PLAYER, A_TECHTIME,
                                    (char[LBUF_SIZE]){0});

  if (olds) {
    if (!parse_time_checked(olds, &old))
      old = context->clock->now;
    if (old < context->clock->now)
      old = context->clock->now;
  } else {
    old = context->clock->now;
  }
  old += added_seconds;
  (void)snprintf(message_buffer, sizeof(message_buffer), "%ld", old);
  silly_atr_set_in(context->database, PLAYER, A_TECHTIME, message_buffer);
  tech_status(&(TechStatusRequest){
      .context = context, .player = PLAYER, .completion = old});
  return clamp_intptr_to_int((intptr_t)(old - context->clock->now));
}

int tech_time_scaled_seconds(BtechContext *context, int units) {
  if (units <= 0)
    return 0;
  double seconds = (double)units * TECH_TICK *
                   btech_context_technology_time_multiplier(context);
  if (seconds <= 0.0)
    return 0;
  if (seconds >= INT_MAX)
    return INT_MAX;
  return max(1, (int)llround(seconds));
}

TechPartParseResult tech_part_parse(const TechPartParseRequest *request) {
  TechPartParseResult result = {};
  Mech *mech = request->mech;
  char *args[5];
  int l;
  int argc;
  int isrear = 0;

  argc = mech_parseattributes(request->text, args, 4);
  if (!argc)
    return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
  if (argc > (2 + request->parse_extra))
    return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
  if (!request->allow_rear) {
    if ((!request->parse_extra && argc != (1 + request->parse_position)) ||
        (request->parse_extra && (argc < (1 + request->parse_position) ||
                                  argc > (2 + request->parse_position))))
      return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
  } else {
    if (argc == 2) {
      if (ascii_to_upper(*checked_string_suffix(args[1], 0)) != 'R')
        return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
      isrear = 8;
    }
  }
  result.location = armor_section_from_string(
      mech_class(mech), mech_movement_type(mech), args[0]);
  if (result.location < 0)
    return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
  if (request->allow_rear)
    result.location += isrear;
  if (request->parse_position) {
    if (!parse_int_checked(args[1], &l))
      return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID};
    l--;
    if (l < 0 || l >= mech_section_critical_count(mech, result.location))
      return (TechPartParseResult){.status = TECH_PART_PARSE_INVALID_POSITION};
    result.position = l;
  }
  if (request->parse_extra) {
    if (argc > 2)
      result.extra = (unsigned char)args[2][0];
  }
  return result;
}

int tech_parsegun(Mech *mech, char *buffer, int *loc, int *pos, int *brand) {
  char *args[3];
  int l;
  int argc;
  int t;
  int c = 0;
  int pi;
  int pb;
  int position;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc < 1 || argc > (2 + (brand != nullptr)))
    return -1;
  if (argc == (2 + (brand != nullptr)) ||
      (brand && argc == 2 && parse_int_checked(args[1], &position) &&
       position != 0)) {
    *loc = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                     args[0]);
    if (*loc < 0)
      return -1;
    if (!parse_int_checked(args[1], &l))
      return -1;
    if (l <= 0 || l > mech_section_critical_count(mech, *loc))
      return -4;
    *pos = l - 1;
  } else {
    /* Check if it's a number */
    if (!parse_int_checked(args[0], &l))
      return -1;
    if (l < 0)
      return -1;
    WeaponNumberLookupResult lookup = weapon_number_find(
        &(WeaponNumberLookupRequest){.mech = mech, .number = l});
    if (!lookup.found)
      return -1;
    *loc = lookup.slot.section;
    *pos = lookup.slot.critical;
  }
  t = mech_critical_part_type(mech, *loc, *pos);
  char **last_argument_slot = (char **)checked_storage_at(
      (void *)args, (size_t)argc, sizeof(*args), (size_t)(argc - 1));
  if (brand != nullptr && argc > 1 &&
      (!parse_int_checked(*last_argument_slot, &position) || position == 0)) {
    PartMatchResult match =
        part_match_next(&(PartMatchRequest){.context = mech_context(mech),
                                            .pattern = *last_argument_slot,
                                            .kind = PART_MATCH_LONG,
                                            .cursor = c});
    if (!match.found)
      return -2;
    pi = match.part.id;
    pb = match.part.brand;
    if (pi != t)
      return -3;
    *brand = pb;
  } else if (brand != nullptr) {
    *brand = mech_critical_brand(mech, *loc, *pos);
  }
  return 0;
}

typedef struct LatestTechEventContext LatestTechEventContext;
struct LatestTechEventContext {
  int latest;
};

static void find_latest_tech_event(MuxEvent *event, void *data) {
  LatestTechEventContext *context = data;
  int offset = event->tick - event->scheduler->tick;
  long amount = ((((long)event->data2) % PLAYERPOS) / 16) - 1;

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
  for (MechEventType type = FIRST_TECH_EVENT; type <= LAST_TECH_EVENT; type++)
    mech_event_visit(mech, type, find_latest_tech_event, &latest);
  return latest.latest;
}
