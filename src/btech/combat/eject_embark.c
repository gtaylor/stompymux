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

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech_channel.h"
#include "btech_event.h"
#include "mech_lifecycle.h"
#include "mech_template_api.h"
#include "mux/commands/action_messages.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
/* Ejection code */
#include "autopilot.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_position_api.h"
#include "mech_radio_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "registry_internal.h"
#include "section_types.h"

void mech_embark(DbRef player, void *data, char *buffer) {

  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  Mech *target, *towee = nullptr;
  int tmp;
  DbRef target_num;
  int argc;
  char *args[4];
  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  if (player != GOD)
    cch(MECH_USUAL);
  if (mech_class(mech) == CLASS_MW) {
    argc = mech_parseattributes(buffer, args, 1);
    DOCHECK_CONTEXT(mech_context(mech), argc != 1,
                    "Invalid number of arguements.");
    target_num = FindTargetDBREFFromMapNumber(mech, args[0]);
    DOCHECK_CONTEXT(mech_context(mech), target_num == -1,
                    "That target is not in your line of sight.");
    target = btech_context_get_mech(mech_context(mech), target_num);
    DOCHECK_CONTEXT(mech_context(mech),
                    !target ||
                        !mech_los_check(mech, target, mech_position_x(target),
                                        mech_position_y(target),
                                        mech_range_to(mech, target)),
                    "That target is not in your line of sight.");
    DOCHECK_CONTEXT(mech_context(mech), mech_cocoon_integrity(target),
                    "You should wait for your target to land first");
    DOCHECK_CONTEXT(mech_context(mech),
                    mech_position_z(mech) > (mech_position_z(target) + 1),
                    "You are too high above the target.");
    DOCHECK_CONTEXT(mech_context(mech),
                    mech_position_z(mech) < (mech_position_z(target) - 1),
                    "You can't reach that high !");
    DOCHECK_CONTEXT(mech_context(mech),
                    mech_position_x(mech) != mech_position_x(target) ||
                        mech_position_y(mech) != mech_position_y(target),
                    "You need to be in the same hex!");
    DOCHECK_CONTEXT(
        mech_context(mech),
        (!is_in_character(btech_context_database(mech_context(mech)),
                          mech_dbref(mech))) ||
            (!is_in_character(btech_context_database(mech_context(mech)),
                              mech_dbref(target))),
        "You don't really see a way to get in there.");
    DOCHECK_CONTEXT(
        mech_context(mech),
        (mech_class(target) == CLASS_VEH_GROUND ||
         mech_class(target) == CLASS_VTOL) &&
            !unit_is_fixable(target),
        "You can't find and entrance amid the mass of twisted metal.");

    if (!lock_test(evaluation, player, player, mech_dbref(mech),
                   mech_dbref(target), LUA_LOCK_ENTER,
                   LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock,
                   &lock_result)) {

      /* Trigger FAIL & AFAIL */
      memset(fail_mesg, 0, sizeof(fail_mesg));
      snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

      notify_lock_failure(evaluation, &lock, &lock_result, fail_mesg, nullptr,
                          LUA_EVENT_FAIL);

      return;
    }

    if (!lock_result.defined) {

      /* Check their teams */
      DOCHECK_CONTEXT(mech_context(mech), mech_team(mech) != mech_team(target),
                      "Locked. Damn !");
    }

    DOCHECK_CONTEXT(mech_context(mech), fabs(mech_current_speed(target)) > 15.,
                    "Are you suicidal ? That thing is moving too fast !");

    if (mech_class(target) == CLASS_MECH) {
      DOCHECK_CONTEXT(
          mech_context(mech), !mech_section_internal(target, HEAD),
          "Okay, just climb up to-- Wait... where did the head go??");
      DOCHECK_CONTEXT(mech_context(mech),
                      mech_critical_is_destroyed(target, HEAD, 2),
                      "Okay, just climb up and open-- "
                      "WTF ? Someone stole the cockpit!");
      DOCHECK_CONTEXT(
          mech_context(mech), mech_critical_is_nonfunctional(target, HEAD, 2),
          "Okay, just climb up and open-- hey, this door won't budge!");
    }
    mech_notify(mech, MECHALL,
                tprintf("You climb into %s.", mech_display_id(target).text));
    mech_los_broadcast(
        mech, tprintf("climbs into %s.", mech_display_id(target).text));
    tele_contents(mech_context(mech), mech_dbref(mech), mech_dbref(target),
                  TELE_ALL);
    discard_mw(mech);
    return;
  }
  /* What heppens with a Bsuit squad? */
  /* Check if the vechile has cargo capacity, or is an Omni Mech */
  argc = mech_parseattributes(buffer, args, 1);
  DOCHECK_CONTEXT(mech_context(mech), argc != 1,
                  "Invalid number of arguements.");
  target_num = FindTargetDBREFFromMapNumber(mech, args[0]);
  DOCHECK_CONTEXT(mech_context(mech), target_num == -1,
                  "That target is not in your line of sight.");
  target = btech_context_get_mech(mech_context(mech), target_num);
  DOCHECK_CONTEXT(mech_context(mech),
                  !target ||
                      !mech_los_check(mech, target, mech_position_x(target),
                                      mech_position_y(target),
                                      mech_range_to(mech, target)),
                  "That target is not in your line of sight.");
  DOCHECK_CONTEXT(mech_context(mech), mech_carried_dbref(mech) == target_num,
                  "You cannot embark what your towing!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_condition_summary(mech).fallen ||
                      mech_event_count(mech, EVENT_STAND),
                  "Help! I've fallen and I can't get up!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !mech_is_started(mech) || mech_is_destroyed(mech),
                  "Ha Ha Ha.");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_jumping(mech),
                  "You cannot do that while jumping!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_jumping(target),
                  "You cannot do that while it is jumping!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_technology_flags_secondary(mech) & CARRIER_TECH &&
                      (mech_is_dropship(target) ? mech_is_dropship(mech) : 1),
                  "You're a bit bulky to do that yourself.");
  DOCHECK_CONTEXT(mech_context(mech), mech_condition_summary(mech).hidden,
                  "You cannot embark while hidden.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_tonnage(mech) > mech_carrier_maximum_tonnage(target),
                  "You are too large for that class of carrier.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_class(mech) != CLASS_BSUIT &&
                      !(mech_technology_flags_secondary(target) & CARRIER_TECH),
                  "This unit can't handle your mass.");
  DOCHECK_CONTEXT(mech_context(mech), mech_maximum_speed(mech) < MP1,
                  "You are to overloaded to enter.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_position_z(mech) > (mech_position_z(target) + 1),
                  "You are too high above the target.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_position_z(mech) < (mech_position_z(target) - 1),
                  "You can't reach that high !");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_position_x(mech) != mech_position_x(target) ||
                      mech_position_y(mech) != mech_position_y(target),
                  "You need to be in the same hex!");

  if (!lock_test(evaluation, player, player, mech_dbref(mech),
                 mech_dbref(target), LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result)) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

    notify_lock_failure(evaluation, &lock, &lock_result, fail_mesg, nullptr,
                        LUA_EVENT_FAIL);

    return;
  }

  if (!lock_result.defined) {

    /* Check their teams */
    DOCHECK_CONTEXT(mech_context(mech), mech_team(mech) != mech_team(target),
                    "Locked. Damn !");
  }

  DOCHECK_CONTEXT(mech_context(mech), fabs(mech_current_speed(target)) > 0,
                  "Are you suicidal ? That thing is moving too fast !");
  DOCHECK_CONTEXT(
      mech_context(mech),
      !is_in_character(btech_context_database(mech_context(mech)),
                       mech_dbref(mech)) ||
          !is_in_character(btech_context_database(mech_context(mech)),
                           mech_dbref(target)),
      "You don't really see a way to get in there.");

  /* New message system for when someone tries to embark
   * but their sections are still cycling (or weapons) */
  if ((tmp = mech_recycling_state(mech, CHECK_BOTH))) {

    if (tmp == 1) {
      notify(evaluation, player, "You have weapons recycling!");
    } else if (tmp == 2) {
      notify(evaluation, player,
             "You are still recovering from your previous action!");
    } else {
      notify(evaluation, player, "error");
    }
    return;
  }

  DOCHECK_CONTEXT(mech_context(mech),
                  (mech_tonnage(mech) * 100) > mech_cargo_space(target),
                  "Not enough cargospace for you!");
  if (mech_carried_dbref(mech) > 0) {
    DOCHECK_CONTEXT(mech_context(mech),
                    !(towee = btech_context_get_mech(mech_context(mech),
                                                     mech_carried_dbref(mech))),
                    "Internal error caused by towed unit! Contact a wizard!");
    DOCHECK_CONTEXT(mech_context(mech),
                    mech_tonnage(towee) > mech_carrier_maximum_tonnage(target),
                    "Your towed unit is  too large for that class of carrier.");
    DOCHECK_CONTEXT(mech_context(mech),
                    ((mech_tonnage(mech) + mech_tonnage(towee)) * 100) >
                        mech_cargo_space(target),
                    "Not enough cargospace for you and your towed unit!");
  }
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_notify(mech, MECHALL,
                tprintf("You climb into %s.", mech_display_id(target).text));
    mech_los_broadcast(
        mech, tprintf("climbs into %s.", mech_display_id(target).text));
  } else {
    mech_notify(mech, MECHALL,
                tprintf("You climb up the entry ramp into %s.",
                        mech_display_id(target).text));
    mech_los_broadcast(mech, tprintf("climbs up the entry ramp into %s.",
                                     mech_display_id(target).text));
    if (towee && mech_carried_dbref(mech) > 0) {
      mech_notify(towee, MECHALL,
                  tprintf("You are drug up the entry ramp into %s.",
                          mech_display_id(target).text));
      mech_los_broadcast(towee, tprintf("is drug up the entry ramp into %s.",
                                        mech_display_id(target).text));
    }
  }
  MarkForLOSUpdate(mech);
  MarkForLOSUpdate(target);

  if (mech_condition_summary(target).hidden) {
    mech_hidden_set(target, false);
    mech_los_broadcast(target, "becomes visible as it is embarked into.");
  }

  /* Check if the unit is towing something so the towed unit
   * is handled first because mech_power_down() will cause it to drop
   * whatever its towing */
  if (towee && mech_carried_dbref(mech) > 0) {
    MarkForLOSUpdate(towee);
    mech_Rsetmapindex(GOD, (void *)towee, tprintf("%d", (int)-1));
    mech_Rsetxy(GOD, (void *)towee, tprintf("%d %d", 0, 0));
    move_via_teleport(evaluation, mech_dbref(towee), mech_dbref(target), 1, 0);
    mech_cargo_space_remove(target, mech_tonnage(towee) * 100);
    mech_power_down(towee);
    mech_carried_dbref_set(mech, -1);
    mech_towed_clear(towee);
  }

  /* Now handle the unit itself */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)-1));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", 0, 0));
  move_via_teleport(evaluation, mech_dbref(mech), mech_dbref(target), 1, 0);
  mech_cargo_space_remove(target, mech_tonnage(mech) * 100);
  mech_power_down(mech);

  mech_speed_correct(target);
}

void autoeject(DbRef player, Mech *mech, int tIsBSuit) {
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

  /* If we're not IC, return */
  if (!player || !is_in_character(database, mech_dbref(mech)) ||
      !btech_context_in_character_enabled(mech_context(mech)) ||
      !is_in_character(database,
                       game_object_location(database, mech_dbref(mech))))
    return;

  /* Create the MW object */
  suit = create_obj(
      evaluation, GOD, OBJECT_TYPE_THING,
      tprintf("MechWarrior - %s", game_object_name(database, player)));
  silly_atr_set_in(database, suit, A_XTYPE, "MECH");
  s_xcode(database, suit);
  btech_special_object_flag_changed(mech_context(mech), GOD, suit, 0, 1);
  d = btech_attribute_read(database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech_context(mech), suit))) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Unable to create special obj for #%ld's ejection.", player));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't create RS object)");
    return;
  }
  if (!mech_template_load(
          GOD, m, (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Unable to load mechwarrior template for #%ld's ejection. (%s)",
                player, (!d || !*d) ? "Default template" : d));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't load MWTemplate)");
    return;
  }
  silly_atr_set_in(database, suit, A_MECHNAME, "MechWarrior");
  mech_team_set(m, mech_team(mech));
  mech_Rsetmapindex(GOD, (void *)m, tprintf("%ld", mech_map_dbref(mech)));
  mech_Rsetxy(GOD, (void *)m,
              tprintf("%d %d", mech_position_x(mech), mech_position_y(mech)));
  mech_Rsetteam(GOD, (void *)m, tprintf("%d", mech_team(mech)));

  /* Tele the MW to the map and player to the MW */
  move_via_teleport(evaluation, suit, mech_map_dbref(mech), 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);

  /* Init the sucker */
  s_in_character(database, suit);
  initialize_pc(player, m);
  mech_pilot_dbref_set(m, player);
  mech_team_set(m, mech_team(mech));
  /* MUDCONF THIS LATER (and fix not copying digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT
  */
  mech_radio_frequency_set(m, 0, random() % 1000000);
  notify(evaluation, player,
         tprintf("Emergency radio channel set to %d.",
                 mech_radio_frequency(m, 0)));
  /* #endif
  #endif
  */

  if (tIsBSuit) {
    mech_los_broadcast(m, "climbs out of one of the destroyed suits!");
    notify(evaluation, player, "You climb out of the unit!");
  } else {
    mech_los_broadcast(m,
                       tprintf("ejected from %s!", mech_display_id(mech).text));
    mech_ood_initiate(
        player, m,
        tprintf("%d %d %d", mech_position_x(m), mech_position_y(m), 150));
    notify(evaluation, player, "You eject from the unit!");
  }
}
