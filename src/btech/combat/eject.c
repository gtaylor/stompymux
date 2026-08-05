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

int tele_contents(BtechContext *context, DbRef from, DbRef to, int flag) {
  DbRef i, tmpnext;
  int count = 0;
  EvaluationContext *evaluation = btech_context_evaluation(context);
  GameDatabase *database = btech_context_database(context);

  SAFE_DOLIST(database, i, tmpnext, game_object_contents(database, from))
  if ((flag & TELE_ALL) || !is_wizard(database, i)) {
    if (flag & TELE_XP && !is_wizard(database, i))
      if (!(is_quiet(database, from)))
        lower_xp(context, i, btech_context_experience_loss(context));
    move_via_teleport(evaluation, i, to, 1, flag & TELE_LOUD ? 0 : 7);
    count++;
  }
  return count;
}

/* Delayed blast event, for various reasons */
static void mech_discard_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));
  DbRef i = mech_dbref(mech);

  /* We'll silently move the mech off, but lets trigger the aleave/oleave/leave
   * of the loc as well before we do anything fancy */
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_LEAVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = game_object_location(database, i),
                                .enactor = i,
                                .cause = i,
                                .source = game_object_location(database, i),
                                .destination = NOTHING},
                    .event = LUA_EVENT_LEAVE});
  c_xcode(database, i);
  btech_special_object_flag_changed(mech_context(mech), GOD, i, 1, 0);
  s_going(database, i);
  s_dark(database, i);
  s_zombie(database, i);
  move_via_teleport(evaluation, i,
                    btech_context_used_mech_store_dbref(mech_context(mech)), 1,
                    7);
}

void discard_mw(Mech *mech) {
  if (is_in_character(btech_context_database(mech_context(mech)),
                      mech_dbref(mech)))
    mech_event_schedule(mech, EVENT_NUKEMECH, mech_discard_event, 10, 0);
}

void enter_mw_bay(Mech *mech, DbRef bay) {
  tele_contents(mech_context(mech), mech_dbref(mech), bay,
                TELE_ALL); /* Even immortals must get going */
  discard_mw(mech);
}

void pickup_mw(Mech *mech, Mech *target) {
  DbRef mw;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  mw = game_object_contents(btech_context_database(mech_context(mech)),
                            mech_dbref(target));
  DOCHECKMA((mech_class(mech) != CLASS_MECH) &&
                (mech_class(mech) != CLASS_VEH_GROUND) &&
                (mech_class(mech) != CLASS_VTOL) &&
                !(mech_technology_flags(mech) & SALVAGE_TECH),
            "You can't pick up, period.")
  if (mw > 0)
    notify_printf(evaluation, mw,
                  "%s scoops you up and brings you into the cockpit.",
                  mech_to_mech_display_id(target, mech).text);
  /* Put the player in the picker uppper and clear him from the map */
  mech_los_broadcast(mech,
                     tprintf("picks up %s.", mech_display_id(target).text));
  mech_printf(mech, MECHALL,
              "You pick up the stray mechwarrior from the field.");
  if (mech_team(target) != mech_team(mech))
    if (btech_context_mechwarrior_pickup_triggers_actions(mech_context(mech)))
      tele_contents(mech_context(mech), mech_dbref(target), mech_dbref(mech),
                    TELE_ALL | TELE_LOUD);
    else
      tele_contents(mech_context(mech), mech_dbref(target), mech_dbref(mech),
                    TELE_ALL);
  else if (btech_context_mechwarrior_pickup_triggers_actions(
               mech_context(mech)))
    tele_contents(mech_context(mech), mech_dbref(target), mech_dbref(mech),
                  TELE_ALL | TELE_LOUD);
  else
    tele_contents(mech_context(mech), mech_dbref(target), mech_dbref(mech),
                  TELE_ALL);
  discard_mw(target);
}

static void char_eject(DbRef player, Mech *mech) {
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

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
  if (!mech_loadnew(GOD, m,
                    (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
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
  move_via_teleport(evaluation, suit, mech_map_dbref(mech), 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);
  mech_los_broadcast(m,
                     tprintf("ejected from %s!", mech_display_id(mech).text));
  s_in_character(database, suit);
  initialize_pc(player, m);
  silly_atr_set_in(database, mech_dbref(m), A_PILOTNUM,
                   tprintf("#%ld", player));
  mech_pilot_dbref_set(m, player);
  mech_team_set(m, mech_team(mech));
  /* MUDCONF THIS LATER (and to not copy digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT

  */
  mech_radio_frequency_set(m, 0, random() % 1000000);
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                mech_radio_frequency(m, 0));
  /* #endif
  #endif
  */
  notify(evaluation, player, "You eject from the unit!");
  if (mech_class(mech) == CLASS_MECH) {
    mech_critical_destroy(mech, HEAD, 2);
  }
  if (!mech_is_destroyed(mech)) {
    mech_destroy(mech, mech, 0, KILL_TYPE_EJECT);
  }
}

void mech_eject(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALS);
  DOCHECK_CONTEXT(mech_context(mech), mech_is_dropship(mech),
                  "Dropships do not support ejection.");
  DOCHECK_CONTEXT(mech_context(mech),
                  !((mech_class(mech) == CLASS_MECH) ||
                    (mech_class(mech) == CLASS_VTOL) ||
                    (mech_class(mech) == CLASS_VEH_GROUND)),
                  "This unit has no ejection seat!");
  DOCHECK_CONTEXT(
      mech_context(mech), mech_is_flying_type(mech) && !mech_is_landed(mech),
      "Regrettably, right now you can only eject when landed, sorry - no "
      "parachute :P");
  DOCHECK_CONTEXT(mech_context(mech),
                  !is_in_character(btech_context_database(mech_context(mech)),
                                   mech_dbref(mech)),
                  "This unit isn't in character!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !btech_context_in_character_enabled(mech_context(mech)),
                  "This MUX isn't in character!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !is_in_character(btech_context_database(mech_context(mech)),
                                   game_object_location(btech_context_database(
                                                            mech_context(mech)),
                                                        mech_dbref(mech))),
                  "Your location isn't in character!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_is_started(mech) && mech_pilot_dbref(mech) != player,
                  "You aren't in da pilot's seat - no ejection for you!");
  if (!mech_is_started(mech)) {
    DOCHECK_CONTEXT(
        mech_context(mech),
        (char_lookupplayer(
            mech_context(mech), GOD, GOD, 0,
            btech_attribute_read(btech_context_database(mech_context(mech)),
                                 mech_dbref(mech), A_PILOTNUM,
                                 (char[LBUF_SIZE]){0}))) != player,
        "You aren't the official pilot of this thing. Try 'disembark'");
  }
  if (mech_class(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(
        mech_context(mech), mech_critical_is_nonfunctional(mech, HEAD, 2),
        "The parts of cockpit that control ejection are already used. Try "
        "'disembark'");
  /* Ok.. time to eject ourselves */
  char_eject(player, mech);
}

static void char_disembark(DbRef player, Mech *mech) {
  Mech *m;
  DbRef suit;
  char *d;
  BattleMap *mymap;
  long initial_speed;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));

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
        tprintf("Unable to create special obj for #%ld's disembarkation.",
                player));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't create RS object)");
    return;
  }
  if (!mech_loadnew(GOD, m,
                    (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("Unable to load mechwarrior template for #%ld's "
                               "disembarkation. (%s)",
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
  mech_position_hex_z_set(m, mech_position_z(mech));
  mech_Rsetteam(GOD, (void *)m, tprintf("%d", mech_team(mech)));
  move_via_teleport(evaluation, suit, mech_map_dbref(mech), 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);
  s_in_character(database, suit);
  initialize_pc(player, m);
  mech_pilot_dbref_set(m, player);
  silly_atr_set_in(database, mech_dbref(m), A_PILOTNUM,
                   tprintf("#%ld", player));
  mech_team_set(m, mech_team(mech));
  /* MUDCONF THIS LATER AND FIX (to not copy digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT
  */
  mech_radio_frequency_set(m, 0, random() % 1000000);
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                mech_radio_frequency(m, 0));
  /* #endif
  #endif
  */
  mymap = btech_context_get_map(mech_context(mech), mech_map_dbref(m));
  if ((mech_position_z(m) > (battle_map_hex_elevation(mymap, mech_position_x(m),
                                                      mech_position_y(m)) +
                             1)) &&
      (mech_position_z(m) > 0)) {
    notify(evaluation, player,
           "You open the hatch and climb out of the unit. Maybe you should "
           "have done this while the thing was closer to the ground...");
    mech_los_broadcast(m, tprintf("jumps out of %s... in mid air !",
                                  mech_display_id(mech).text));
    initial_speed =
        ((mech_current_speed(mech) + mech_vertical_speed(mech)) / MP1) / 2 + 4;
    mech_event_schedule(m, EVENT_FALL, mech_fall_event, FALL_TICK,
                        -initial_speed);
  } else {
    mech_los_broadcast(
        m, tprintf("climbs out of %s!", mech_display_id(mech).text));
    notify(evaluation, player, "You climb out of the unit.");
  }
}

/**
 * Handle the disembarking of pilots from units.
 */
void mech_disembark(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALS);
  DOCHECK_CONTEXT(
      mech_context(mech),
      !((mech_class(mech) == CLASS_MECH) || (mech_class(mech) == CLASS_VTOL) ||
        (mech_class(mech) == CLASS_VEH_GROUND)),
      "The door ! The door ? The Door ?!? Where's the exit in this damned "
      "thing ?");

  /*  DOCHECK_CONTEXT(mech->xcode.context, FlyingT(mech) && !Landed(mech),
   * "What, in the air ? Are you suicidal ?"); */
  DOCHECK_CONTEXT(mech_context(mech),
                  !is_in_character(btech_context_database(mech_context(mech)),
                                   mech_dbref(mech)),
                  "This unit isn't in character!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !btech_context_in_character_enabled(mech_context(mech)),
                  "This MUX isn't in character!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !is_in_character(btech_context_database(mech_context(mech)),
                                   game_object_location(btech_context_database(
                                                            mech_context(mech)),
                                                        mech_dbref(mech))),
                  "Your location isn't in character!");
  DOCHECK_CONTEXT(
      mech_context(mech),
      (mech_is_started(mech) || mech_event_count(mech, EVENT_STARTUP)) &&
          (mech_pilot_dbref(mech) == player),
      "While it's running!? Don't be daft.");
  DOCHECK_CONTEXT(mech_context(mech), fabs(mech_current_speed(mech)) > 25.,
                  "Are you suicidal ? That thing is moving too fast !");
  /* Ok.. time to disembark ourselves */
  char_disembark(player, mech);
}

/**
 * Handle the disembarking of units from within carriers.
 */
void mech_udisembark(DbRef player, void *data, char *buffer) {

  Mech *mech = (Mech *)data; /* The disembarking unit */
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  GameDatabase *database = btech_context_database(mech_context(mech));
  Mech *target;
  int newmech;       /* The carrier. */
  BattleMap *mymap;  /* The map to disembark to */
  int under_repairs; /* Is the unit still under repairs? */
  int i;             /* Used in section recycle for loop. */

  /* Any IN_CHARACTER unit's pilot must match the invoker to disembark.
   * A unit that is not IC can be disembarked by anyone.
   */
  DOCHECK_CONTEXT(
      mech_context(mech),
      is_in_character(database, mech_dbref(mech)) &&
          !is_wizard(database, player) &&
          (char_lookupplayer(
               mech_context(mech), GOD, GOD, 0,
               btech_attribute_read(database, mech_dbref(mech), A_PILOTNUM,
                                    (char[LBUF_SIZE]){0})) != player),
      "This isn't your mech!");

  /* Find the carrier that the invoker's unit is in and check it for validity.
   */
  newmech = game_object_location(database, mech_dbref(mech));
  DOCHECK_CONTEXT(
      mech_context(mech),
      !(is_good_obj(database, newmech) && is_xcode(database, newmech)),
      "You're not being carried!");
  DOCHECK_CONTEXT(
      mech_context(mech),
      !(target = btech_context_get_mech(mech_context(mech), newmech)),
      "Not being carried!");
  DOCHECK_CONTEXT(mech_context(mech), mech_map_dbref(target) == -1,
                  "You are not on a map.");

  /* Don't allow repairing units to disembark */
  under_repairs = figure_latest_tech_event(mech);
  DOCHECK_CONTEXT(
      mech_context(mech), under_repairs,
      "This 'Mech is still under repairs (see checkstatus for more info)");

  DOCHECK_CONTEXT(
      mech_context(mech),
      fabs(mech_current_speed(target)) > mech_walking_speed(target),
      "You cannot leave while the carrier is moving faster than walk speed!");

  /* Carry out the disembarking. */
  mech_Rsetmapindex(GOD, (void *)mech,
                    tprintf("%d", (int)mech_map_dbref(target)));
  mech_Rsetxy(
      GOD, (void *)mech,
      tprintf("%d %d", mech_position_x(target), mech_position_y(target)));
  mech_position_z_set(mech, mech_position_z(target));
  mech_position_real_z_set(mech, ZSCALE * mech_position_z(mech));
  mymap = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  DOCHECK_CONTEXT(mech_context(mech), !mymap,
                  "Major map error possible. Prolly should contact a wizard.");

  /* Teleport loudly so native enter events and other messages run. */
  move_via_teleport(evaluation, mech_dbref(mech), mech_map_dbref(mech), 1, 0);

  /* If we make it safely, start the invoker's unit up once it's on the map. */
  if (!mech_is_destroyed(mech) &&
      game_object_location(database, player) == mech_dbref(mech)) {
    mech_pilot_dbref_set(mech, player);
    mech_power_up(mech);
  }

  MarkForLOSUpdate(mech);
  mech_cargo_weight_recalculate(mech);
  mech_player_killer_set(mech, false);
  mech_los_broadcast(mech, "powers up!");
  mech_sixth_sense_set(
      mech, ((mech_pilot_dbref(mech) > 0 &&
              is_player(database, mech_pilot_dbref(mech)))
                 ? char_getvalue(mech_context(mech), mech_pilot_dbref(mech),
                                 "Sixth_Sense")
                 : 0));
  mech_communication_skill_set(mech, DEFAULT_COMM);

  if (is_player(database, mech_pilot_dbref(mech)) &&
      !is_quiet(database, mech_dbref(mech))) {
    mech_communication_skill_set(
        mech, char_getskilltarget(mech_context(mech), mech_pilot_dbref(mech),
                                  "Comm-Conventional", 0));
    mech_perception_target_set(mech, char_getskilltarget(mech_context(mech),
                                                         mech_pilot_dbref(mech),
                                                         "Perception", 0));
  } else {
    mech_communication_skill_set(mech, 6);
    mech_perception_target_set(mech, 6);
  }
  mech_communication_last_tick_set(mech, 0);
  autopilot_resume_for_mech(mech);
  mech_cargo_space_add(target, mech_tonnage(mech) * 100);
  MarkForLOSUpdate(target);

  /* A hidden carrier that is disembarked from loses its HIDDEN status */
  if (mech_condition_summary(target).hidden) {
    mech_hidden_set(target, false);
    mech_los_broadcast(target, "becomes visible as it is disembarked from.");
  }

  /* Para-dropping out of units from elevations. */
  if (!mech_is_flying_type(mech) &&
      mech_position_z(mech) > battle_map_hex_elevation(mymap,
                                                       mech_position_x(mech),
                                                       mech_position_y(mech)) &&
      mech_position_z(mech) > 0) {

    notify(evaluation, player,
           "You open the hatch and drop out of the unit....");
    mech_los_broadcast(
        mech, tprintf("drops out of %s and begins falling to the ground.",
                      mech_display_id(target).text));
    mech_ood_initiate(player, mech,
                      tprintf("%d %d %d", mech_position_x(mech),
                              mech_position_y(mech), mech_position_z(mech)));
  } else {
    if (mech_class(mech) == CLASS_BSUIT) {
      mech_los_broadcast(
          mech, tprintf("climbs out of %s!", mech_display_id(target).text));
      notify(evaluation, player, "You climb out of the unit.");
    } else {
      /* If the carrier is destroyed, do damage to the disembarking unit. */
      if (mech_is_destroyed(target) || !mech_is_started(target)) {
        mech_los_broadcast(
            mech, tprintf("smashes open the ramp door and emerges from %s!",
                          mech_display_id(target).text));
        notify(evaluation, player, "You smash open the door and break out.");
        mech_fall(mech, 4, 0);
      } else {
        /* All is well. */
        mech_los_broadcast(mech, tprintf("emerges from the ramp out of %s!",
                                         mech_display_id(target).text));
        notify(evaluation, player, "You emerge from the unit loading ramp.");
        if (mech_is_landed(mech) &&
            mech_position_z(mech) >
                battle_map_hex_elevation(mymap, mech_position_x(mech),
                                         mech_position_y(mech)) &&
            mech_is_flying_type(mech))
          mech_landed_set(mech, false);
      }
    }
  }

  /* Recycle any weapons/sections they have to prevent munchkin behavior. */
  if (mech_class(mech) == CLASS_BSUIT) {
    bsuit_recycle_start(mech, 20);
  } else if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW) {
    for (i = 0; i < NUM_SECTIONS; i++)
      mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  } else if (mech_class(mech) == CLASS_VEH_GROUND ||
             mech_class(mech) == CLASS_VTOL) {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (i == ROTOR)
        continue;
      else
        mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  }

  fix_pilotdamage(mech, mech_pilot_dbref(mech));
  mech_speed_correct(target);
} /* end mech_udisembark */
