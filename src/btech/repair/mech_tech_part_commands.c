/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "legacy_macros.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "section_types.h"

#define my_parsepart(loc, part)                                                \
  switch (tech_parsepart(mech, buffer, loc, part, NULL)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsepart2(loc, part, brand)                                        \
  switch (tech_parsepart(mech, buffer, loc, part, brand)) {                    \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid section!");                            \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid part!");                               \
    return;                                                                    \
  }

#define my_parsegun(loc, part, brand)                                          \
  switch (tech_parsegun(mech, buffer, loc, part, brand)) {                     \
  case -1:                                                                     \
    notify(evaluation, player, "Invalid gun #!");                              \
    return;                                                                    \
  case -2:                                                                     \
    notify(evaluation, player, "Invalid object to replace with!");             \
    return;                                                                    \
  case -3:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid object type - not matching with original.");               \
    return;                                                                    \
  case -4:                                                                     \
    notify(evaluation, player,                                                 \
           "Invalid gun location - subscript out of range.");                  \
    return;                                                                    \
  }

#define ClanMod(num)                                                           \
  MAX(1, (((num) / ((mech_technology_flags(mech) & CLAN_TECH) ? 2 : 1))))

typedef struct TechCheckContext {
  int matches;
  int location;
  int part;
} TechCheckContext;
TECHCOMMANDH(tech_replacegun) {
  int brand = 0, ob = 0;

  int roll, rollmod, fixtime, base_fixtime, parttype, oparttype, fail_fixtime;

  TECHCOMMANDB;
  TECHCOMMANDC;
  my_parsegun(&loc, &part, &brand);
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(context, !IsWeapon(mech_critical_part_type(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(context, !ValidGunPos(mech, loc, part),
                  "You can't replace middle of a gun!");
  DOCHECK_CONTEXT(context, !mech_critical_is_nonfunctional(mech, loc, part),
                  "That gun isn't hurtin'!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  if (brand) {
    ob = mech_critical_brand(mech, loc, part);
    mech_critical_brand_set(mech, loc, part, brand);
  }

  parttype = oparttype = mech_critical_part_type(mech, loc, part);

  DOCHECK_CONTEXT(
      context,
      IsAmmo(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, mech_critical_brand(mech, loc, part)) < 1,
      tprintf("Not enough units of %s in store.",
              part_name(context, parttype, mech_critical_brand(mech, loc, part))
                  .text));

  notify_printf(evaluation, player, "You start replacing the gun...");
  rollmod = REPLACE_DIFFICULTY +
            WEAPTYPE_DIFFICULTY(mech_critical_part_type(mech, loc, part));
  roll = tech_weapon_roll(player, mech, rollmod);
  base_fixtime = REPLACEGUN_TIME *
                 ClanMod(GetWeaponCrits(
                     mech, Weapon2I(mech_critical_part_type(mech, loc, part))));
  fail_fixtime = (base_fixtime * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the gun...");
    rollmod = REPLACE_DIFFICULTY;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the gun for good...");
      /* part goes , 1.5 * techtime*/
      if (!(IsAmmo(mech_critical_part_type(mech, loc, part))))
        econ_change_items(
            context,
            mech_is_dropship(mech)
                ? mech_bay_dbref(mech, 0)
                : game_object_location(btech_context_database(context),
                                       mech_dbref(mech)),
            parttype, mech_critical_brand(mech, loc, part), -1);
      tech_addtechtime(context, player, fixtime);
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, very_fake_func,
          MAX(1, player_techtime(context, player) * TECH_TICK),
          PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS);

    } else {
      notify_printf(evaluation, player, "You manage to save the gun...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            btech_context_uses_variable_technology_time(context)
                ? (fail_fixtime * 10) /
                      (1000 /
                       (100 - (roll ? btech_context_technology_time_modifier(
                                          context) *
                                          roll
                                    : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(context, player, fixtime);
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPLG, very_fake_func,
          MAX(1, player_techtime(context, player) * TECH_TICK),
          PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS);
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          btech_context_uses_variable_technology_time(context)
              ? (base_fixtime * 10) /
                    (1000 /
                     (100 -
                      (roll ? btech_context_technology_time_modifier(context) *
                                  roll
                            : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");
    if (!(IsAmmo(mech_critical_part_type(mech, loc, part))))
      econ_change_items(
          context,
          mech_is_dropship(mech)
              ? mech_bay_dbref(mech, 0)
              : game_object_location(btech_context_database(context),
                                     mech_dbref(mech)),
          parttype, mech_critical_brand(mech, loc, part), -1);
    tech_addtechtime(context, player, fixtime);
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPLG, mux_event_tickmech_replacegun,
        MAX(1, player_techtime(context, player) * TECH_TICK),
        PACK_LOCPOS_E(loc, part, brand) + player * PLAYERPOS);
  }

  if (brand)
    mech_critical_brand_set(mech, loc, part, ob);
}

TECHCOMMANDH(tech_repairgun) {
  int extra_hard = 0;

  TECHCOMMANDB;
  TECHCOMMANDC;
  /* Find the gun for us */
  my_parsegun(&loc, &part, NULL);
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(context, !IsWeapon(mech_critical_part_type(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(context, !ValidGunPos(mech, loc, part),
                  "You can't repair middle of a gun!");
  DOCHECK_CONTEXT(context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(context, mech_critical_is_disabled(mech, loc, part),
                  "That gun can't be fixed yet!");

  if (mech_critical_is_destroyed(mech, loc, part)) {
    if (GetWeaponCrits(
            mech, Weapon2I(mech_critical_part_type(mech, loc, part))) < 5 ||
        mech_critical_is_destroyed(mech, loc, part + 1)) {
      notify(evaluation, player, "That gun is gone for good!");
      return;
    }
    extra_hard = 1;
  } else if (!mech_critical_temporary_failure(mech, loc, part)) {
    notify(evaluation, player, "That gun isn't hurtin'!");
    return;
  }

  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  DOTECH_LOCPOS(
      REPAIR_DIFFICULTY +
          WEAPTYPE_DIFFICULTY(mech_critical_part_type(mech, loc, part)) +
          extra_hard,
      repairg_fail, repairg_succ, repair_econ, REPAIRGUN_TIME, mech,
      PACK_LOCPOS(loc, part), mux_event_tickmech_repairgun, EVENT_REPAIR_REPAP,
      "You start repairing the weapon..", 1);
}

TECHCOMMANDH(tech_fixenhcrit) {
  TECHCOMMANDB;
  TECHCOMMANDC;
  /* Find the gun for us */
  my_parsegun(&loc, &part, NULL);
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(context, !IsWeapon(mech_critical_part_type(mech, loc, part)),
                  "That's no gun!");
  DOCHECK_CONTEXT(context, SomeoneScrappingPart(mech, loc, part),
                  "Someone's scrapping it already!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(context, mech_critical_is_disabled(mech, loc, part),
                  "That gun can't be fixed yet!");

  if (!mech_critical_is_damaged(mech, loc, part)) {
    notify(evaluation, player, "That gun isn't damaged!");
    return;
  }

  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  DOTECH_LOCPOS(ENHCRIT_DIFFICULTY, repairenhcrit_fail, repairenhcrit_succ,
                repairenhcrit_econ, REPAIRENHCRIT_TIME, mech,
                PACK_LOCPOS(loc, part), mux_event_tickmech_repairenhcrit,
                EVENT_REPAIR_REPENHCRIT, "You start repairing the weapon...",
                1);
}

TECHCOMMANDH(tech_replacepart) {
  TECHCOMMANDB;

  TECHCOMMANDC;

  int roll, rollmod, fixtime, base_fixtime, parttype, oparttype, fail_fixtime;

  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(context,
                  (t = mech_critical_part_type(mech, loc, part)) == EMPTY,
                  "That location is empty!");
  DOCHECK_CONTEXT(context, !mech_critical_is_nonfunctional(mech, loc, part),
                  "That part looks ok to me..");
  DOCHECK_CONTEXT(context,
                  mech_part_is_structural_placeholder(
                      mech_critical_part_type(mech, loc, part)),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(context, IsWeapon(t),
                  "That's a weapon! Use replacegun instead.");
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  /* little cheating here to get the proper part, since we aren't doing complex
   * repairs */
  oparttype = mech_critical_part_type(mech, loc, part);
  parttype =
      (IsActuator(oparttype)
           ? Cargo(S_ACTUATOR)
           : (oparttype == Special(ENGINE)
                  ? ((mech_technology_flags(mech) & XL_TECH) ? Cargo(XL_ENGINE)
                     : (mech_technology_flags(mech) & ICE_TECH)
                         ? Cargo(IC_ENGINE)
                     : (mech_technology_flags(mech) & XXL_TECH)
                         ? Cargo(XXL_ENGINE)
                     : (mech_technology_flags(mech) & CE_TECH)
                         ? Cargo(COMP_ENGINE)
                     : (mech_technology_flags(mech) & LE_TECH)
                         ? Cargo(LIGHT_ENGINE)
                         : oparttype)
                  : (oparttype == Special(HEAT_SINK) &&
                             mech_has_double_heat_sinks(mech)
                         ? Cargo(DOUBLE_HEAT_SINK)
                         : oparttype)));

  DOCHECK_CONTEXT(
      context,
      IsAmmo(mech_critical_part_type(mech, loc, part))
          ? 0
          : econ_find_items(
                context,
                mech_is_dropship(mech)
                    ? mech_bay_dbref(mech, 0)
                    : game_object_location(btech_context_database(context),
                                           mech_dbref(mech)),
                parttype, mech_critical_brand(mech, loc, part)) < 1,
      tprintf("Not enough units of %s in store.",
              part_name(context, parttype, mech_critical_brand(mech, loc, part))
                  .text));

  notify_printf(evaluation, player, "You start replacing the part...");
  rollmod = REPLACE_DIFFICULTY +
            PARTTYPE_DIFFICULTY(mech_critical_part_type(mech, loc, part));
  roll = tech_roll(player, mech, rollmod);
  base_fixtime = REPLACEPART_TIME;
  fail_fixtime = (REPLACEPART_TIME * 3) / 2;

  if (roll < 0) {
    notify_printf(
        evaluation, player,
        "Your attempt is unsuccessful, but you try to save the part...");
    rollmod = rollmod + 1;
    roll = tech_roll(player, mech, rollmod);
    if (roll < 0) {
      fixtime = fail_fixtime;
      notify_printf(evaluation, player,
                    "You muck around, wasting the part for good...");
      /* part goes , 1.5 * techtime*/
      econ_change_items(
          context,
          mech_is_dropship(mech)
              ? mech_bay_dbref(mech, 0)
              : game_object_location(btech_context_database(context),
                                     mech_dbref(mech)),
          parttype, mech_critical_brand(mech, loc, part), -1);
      tech_addtechtime(context, player, fixtime);
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, very_fake_func,
          MAX(1, player_techtime(context, player) * TECH_TICK),
          PACK_LOCPOS(loc, part) + player * PLAYERPOS);

    } else {
      notify_printf(evaluation, player, "You manage to save the part...");
      /* part doesn't go. 1.5 * techtime, but lets mod the fix time if
       * applicable*/
      /* We should really MIN(100,mod * roll) for the subtract to cap this out
       */
      if (roll == 0)
        fixtime = fail_fixtime;
      else
        fixtime =
            btech_context_uses_variable_technology_time(context)
                ? (fail_fixtime * 10) /
                      (1000 /
                       (100 - (roll ? btech_context_technology_time_modifier(
                                          context) *
                                          roll
                                    : 0)))
                : fail_fixtime;
      if (fail_fixtime - fixtime)
        notify_printf(
            evaluation, player, "Your skill manages to save %d minute%s",
            fail_fixtime - fixtime, fail_fixtime - fixtime == 1 ? "!" : "s!");
      tech_addtechtime(context, player, fixtime);
      btech_context_event_schedule(
          context, mech, EVENT_REPAIR_REPL, very_fake_func,
          MAX(1, player_techtime(context, player) * TECH_TICK),
          PACK_LOCPOS(loc, part) + player * PLAYERPOS);
    }

  } else {
    if (roll == 0)
      fixtime = base_fixtime;
    else
      fixtime =
          btech_context_uses_variable_technology_time(context)
              ? (base_fixtime * 10) /
                    (1000 /
                     (100 -
                      (roll ? btech_context_technology_time_modifier(context) *
                                  roll
                            : 0)))
              : base_fixtime;
    if (base_fixtime - fixtime)
      notify_printf(
          evaluation, player, "Your skill manages to save %d minute%s",
          base_fixtime - fixtime, base_fixtime - fixtime == 1 ? "!" : "s!");

    econ_change_items(
        context,
        mech_is_dropship(mech)
            ? mech_bay_dbref(mech, 0)
            : game_object_location(btech_context_database(context),
                                   mech_dbref(mech)),
        parttype, mech_critical_brand(mech, loc, part), -1);
    tech_addtechtime(context, player, fixtime);
    btech_context_event_schedule(
        context, mech, EVENT_REPAIR_REPL, mux_event_tickmech_repairpart,
        MAX(1, player_techtime(context, player) * TECH_TICK),
        PACK_LOCPOS(loc, part) + player * PLAYERPOS);
  }
}

TECHCOMMANDH(tech_repairpart) {
  TECHCOMMANDB;

  TECHCOMMANDC;
  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(context,
                  (t = mech_critical_part_type(mech, loc, part)) == EMPTY,
                  "That location is empty!");
  DOCHECK_CONTEXT(context, mech_critical_is_destroyed(mech, loc, part),
                  "That part is gone for good!");
  DOCHECK_CONTEXT(context, mech_critical_is_disabled(mech, loc, part),
                  "That part can't be repaired yet!");
  DOCHECK_CONTEXT(context, !mech_critical_temporary_failure(mech, loc, part),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(context,
                  mech_part_is_structural_placeholder(
                      mech_critical_part_type(mech, loc, part)),
                  "That part isn't hurtin'!");
  DOCHECK_CONTEXT(context, IsWeapon(t),
                  "That's a weapon! Use repairgun instead.");
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's repairing that part already!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  DOTECH_LOCPOS(REPAIR_DIFFICULTY + PARTTYPE_DIFFICULTY(mech_critical_part_type(
                                        mech, loc, part)),
                repairp_fail, repairp_succ, repair_econ, REPAIRPART_TIME, mech,
                PACK_LOCPOS(loc, part), mux_event_tickmech_repairpart,
                EVENT_REPAIR_REPAP, "You start repairing the part..", 0);
}

TECHCOMMANDH(tech_toggletype) {
  int atype;

  TECHCOMMANDB;

  DOCHECK_CONTEXT(
      context,
      (!is_wizard(btech_context_database(context), player)) &&
          is_in_character(btech_context_database(context), mech_dbref(mech)),
      "This command only works in simpods!");
  my_parsepart2(&loc, &part, &atype);
  DOCHECK_CONTEXT(context,
                  !IsAmmo((t = mech_critical_part_type(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(context,
                  mech_critical_is_nonfunctional(mech, loc, part) ||
                      mech_critical_is_disabled(mech, loc, part),
                  "The ammo compartment is nonfunctional!");
  DOCHECK_CONTEXT(context, !atype,
                  "You need to give a type to toggle to (use - for normal)");
  DOCHECK_CONTEXT(context, (t = (valid_ammo_mode(mech, loc, part, atype))) < 0,
                  "That is invalid ammo type for this weapon!");
  mech_critical_ammo_mode_set(
      mech, loc, part,
      (mech_critical_ammo_mode(mech, loc, part) & ~AMMO_MODES) | t);
  mech_critical_data_set(mech, loc, part, FullAmmo(mech, loc, part));
  mech_notify(mech, MECHALL, "Ammo toggled.");
}

TECHCOMMANDH(tech_reload) {
  int atype;

  TECHCOMMANDB;
  TECHCOMMANDD;
  my_parsepart2(&loc, &part, &atype);
  DOCHECK_CONTEXT(context,
                  !IsAmmo((t = mech_critical_part_type(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(
      context, mech_critical_is_nonfunctional(mech, loc, part),
      "The ammo compartment is destroyed ; repair/replacepart it first.");
  DOCHECK_CONTEXT(
      context, mech_critical_is_disabled(mech, loc, part),
      "The ammo compartment is disabled ; repair/replacepart it first.");
  DOCHECK_CONTEXT(context,
                  (now = mech_critical_data(mech, loc, part)) ==
                      (full = FullAmmo(mech, loc, part)),
                  "That particular ammo compartment doesn't need reloading.");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's playing with that part already!");
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  if (atype) {
    DOCHECK_CONTEXT(context,
                    (t = (valid_ammo_mode(mech, loc, part, atype))) < 0,
                    "That is invalid ammo type for this weapon!");
    mech_critical_data_set(mech, loc, part, 0);
    mech_critical_ammo_mode_set(
        mech, loc, part,
        (mech_critical_ammo_mode(mech, loc, part) & ~AMMO_MODES) | t);
  }
  change = 0;

  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  DOTECH_LOCPOS_VAL(RELOAD_DIFFICULTY, reload_fail, reload_succ, reload_econ,
                    &change, RELOAD_TIME, mech,
                    PACK_LOCPOS_E(loc, part, change), mux_event_tickmech_reload,
                    EVENT_REPAIR_RELO,
                    "You start reloading the ammo compartment..");
}

TECHCOMMANDH(tech_unload) {
  TECHCOMMANDB;

  TECHCOMMANDD;
  my_parsepart(&loc, &part);
  DOCHECK_CONTEXT(context,
                  !IsAmmo((t = mech_critical_part_type(mech, loc, part))),
                  "That's no ammo!");
  DOCHECK_CONTEXT(
      context, mech_critical_is_nonfunctional(mech, loc, part),
      "The ammo compartment is destroyed ; repair/replacepart it first.");
  DOCHECK_CONTEXT(
      context, mech_critical_is_disabled(mech, loc, part),
      "The ammo compartment is disabled ; repair/replacepart it first.");
  DOCHECK_CONTEXT(context, !(now = mech_critical_data(mech, loc, part)),
                  "That particular ammo compartment is empty already.");
  DOCHECK_CONTEXT(context, SomeoneRepairing(mech, loc, part),
                  "Someone's playing with that part already!");
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  if ((full = FullAmmo(mech, loc, part)) == now)
    change = 2;
  else
    change = 1;
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  if (tech_roll(player, mech, REMOVES_DIFFICULTY) < 0)
    mod = 3;
  START("You start unloading the ammo compartment..");
  STARTREPAIR(RELOAD_TIME, mech, PACK_LOCPOS_E(loc, part, change),
              mux_event_tickmech_reload, EVENT_REPAIR_RELO);
}

TECHCOMMANDH(tech_fixarmor) {
  int ochange;

  TECHCOMMANDB;

  TECHCOMMANDD;
  DOCHECK_CONTEXT(
      context, tech_parsepart_advanced(mech, buffer, &loc, NULL, NULL, 1) < 0,
      "Invalid section!");
  if (loc >= 8) {
    from = mech_section_rear_armor(mech, loc % 8);
    to = mech_section_original_rear_armor(mech, loc % 8);
  } else {
    from = mech_section_armor(mech, loc);
    to = mech_section_original_armor(mech, loc);
  }
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc % 8),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc % 8),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context,
                  SomeoneFixingA(mech, loc) || SomeoneFixingI(mech, loc % 8),
                  "Someone's repairing that section already!");
  DOCHECK_CONTEXT(context,
                  mech_section_internal(mech, loc % 8) !=
                      mech_section_original_internal(mech, loc % 8),
                  "The internals need to be fixed first!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  from = MIN(to, from);
  DOCHECK_CONTEXT(context, from == to,
                  "The location doesn't need armor repair!");
  change = to - from;
  ochange = change;
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");
  DOTECH_LOC_VAL_S(FIXARMOR_DIFFICULTY, fixarmor_fail, fixarmor_succ,
                   fixarmor_econ, &change, FIXARMOR_TIME * ochange,
                   EVENT_REPAIR_FIX, mech, loc, "You start fixing the armor..");
  STARTIREPAIR(FIXARMOR_TIME * change, mech, (change * 16 + loc),
               mux_event_tickmech_repairarmor, EVENT_REPAIR_FIX, change);
}

TECHCOMMANDH(tech_fixinternal) {
  TECHCOMMANDB int ochange;

  TECHCOMMANDC;
  my_parsepart(&loc, NULL);
  from = mech_section_internal(mech, loc);
  to = mech_section_original_internal(mech, loc);
  DOCHECK_CONTEXT(context, from == to,
                  "The location doesn't need internals' repair!");
  change = to - from;
  DOCHECK_CONTEXT(context, mech_section_is_destroyed(mech, loc),
                  "That part's blown off! Use reattach first!");
  DOCHECK_CONTEXT(context, mech_section_is_flooded(mech, loc),
                  "That location has been flooded! Use reseal first!");
  DOCHECK_CONTEXT(context, SomeoneFixing(mech, loc),
                  "Someone's repairing that section already!");
  DOCHECK_CONTEXT(
      context, SomeoneScrappingLoc(mech, loc),
      "Someone's scrapping that section - no repairs are possible!");
  ochange = change;
  DOCHECK_CONTEXT(context,
                  player_techtime(context, player) >=
                      btech_context_maximum_technology_time(context),
                  "You're too tired to do that!");

  DOTECH_LOC_VAL_S(FIXINTERNAL_DIFFICULTY, fixinternal_fail, fixinternal_succ,
                   fixinternal_econ, &change, FIXINTERNAL_TIME * ochange,
                   EVENT_REPAIR_FIX, mech, loc,
                   "You start fixing the internals..");
  STARTIREPAIR(FIXINTERNAL_TIME * change, mech, (change * 16 + loc),
               mux_event_tickmech_repairinternal, EVENT_REPAIR_FIXI, change);
}

#define CHECK(tloc, nloc)                                                      \
  case tloc:                                                                   \
    if (mech_section_is_destroyed(mech, nloc))                                 \
      return 1;                                                                \
    break;
