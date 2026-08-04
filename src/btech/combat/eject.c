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
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_restrict_api.h"
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

int tele_contents(BtechContext *context, DbRef from, DbRef to, int flag) {
  DbRef i, tmpnext;
  int count = 0;
  EvaluationContext *evaluation = btech_context_evaluation(context);

  SAFE_DOLIST(context->database, i, tmpnext,
              game_object_contents(context->database, from))
  if ((flag & TELE_ALL) || !is_wizard(context->database, i)) {
    if (flag & TELE_XP && !is_wizard(context->database, i))
      if (!(is_quiet(context->database, from)))
        lower_xp(context, i, context->configuration->btech_xploss);
    move_via_teleport(evaluation, i, to, 1, flag & TELE_LOUD ? 0 : 7);
    count++;
  }
  return count;
}

/* Delayed blast event, for various reasons */
static void mech_discard_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  DbRef i = mech->mynum;

  /* We'll silently move the mech off, but lets trigger the aleave/oleave/leave
   * of the loc as well before we do anything fancy */
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_LEAVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = game_object_location(
                                    mech->xcode.context->database, i),
                                .enactor = i,
                                .cause = i,
                                .source = game_object_location(
                                    mech->xcode.context->database, i),
                                .destination = NOTHING},
                    .event = LUA_EVENT_LEAVE});
  c_xcode(mech->xcode.context->database, i);
  btech_special_object_flag_changed(mech->xcode.context, GOD, i, 1, 0);
  s_going(mech->xcode.context->database, i);
  s_dark(mech->xcode.context->database, i);
  s_zombie(mech->xcode.context->database, i);
  move_via_teleport(evaluation, i,
                    mech->xcode.context->configuration->btech_usedmechstore, 1,
                    7);
}

void discard_mw(Mech *mech) {
  if (is_in_character(mech->xcode.context->database, mech->mynum))
    mech_event_schedule(mech, EVENT_NUKEMECH, mech_discard_event, 10, 0);
}

void enter_mw_bay(Mech *mech, DbRef bay) {
  tele_contents(mech->xcode.context, mech->mynum, bay,
                TELE_ALL); /* Even immortals must get going */
  discard_mw(mech);
}

void pickup_mw(Mech *mech, Mech *target) {
  DbRef mw;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  mw = game_object_contents(mech->xcode.context->database, target->mynum);
  DOCHECKMA((MechType(mech) != CLASS_MECH) &&
                (MechType(mech) != CLASS_VEH_GROUND) &&
                (MechType(mech) != CLASS_VTOL) &&
                !(MechSpecials(mech) & SALVAGE_TECH),
            "You can't pick up, period.")
  if (mw > 0)
    notify_printf(evaluation, mw,
                  "%s scoops you up and brings you into the cockpit.",
                  mech_to_mech_display_id(target, mech).text);
  /* Put the player in the picker uppper and clear him from the map */
  MechLOSBroadcast(mech, tprintf("picks up %s.", mech_display_id(target).text));
  mech_printf(mech, MECHALL,
              "You pick up the stray mechwarrior from the field.");
  if (MechTeam(target) != MechTeam(mech))
    if (mech->xcode.context->configuration->btech_mwpickup_action)
      tele_contents(mech->xcode.context, target->mynum, mech->mynum,
                    TELE_ALL | TELE_LOUD);
    else
      tele_contents(mech->xcode.context, target->mynum, mech->mynum, TELE_ALL);
  else if (mech->xcode.context->configuration->btech_mwpickup_action)
    tele_contents(mech->xcode.context, target->mynum, mech->mynum,
                  TELE_ALL | TELE_LOUD);
  else
    tele_contents(mech->xcode.context, target->mynum, mech->mynum, TELE_ALL);
  discard_mw(target);
}

static void char_eject(DbRef player, Mech *mech) {
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  suit = create_obj(
      evaluation, GOD, OBJECT_TYPE_THING,
      tprintf("MechWarrior - %s",
              game_object_name(mech->xcode.context->database, player)));
  silly_atr_set_in(mech->xcode.context->database, suit, A_XTYPE, "MECH");
  s_xcode(mech->xcode.context->database, suit);
  btech_special_object_flag_changed(mech->xcode.context, GOD, suit, 0, 1);
  d = btech_attribute_read(mech->xcode.context->database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech->xcode.context, suit))) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
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
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Unable to load mechwarrior template for #%ld's ejection. (%s)",
                player, (!d || !*d) ? "Default template" : d));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't load MWTemplate)");
    return;
  }
  silly_atr_set_in(mech->xcode.context->database, suit, A_MECHNAME,
                   "MechWarrior");
  MechTeam(m) = MechTeam(mech);
  mech_Rsetmapindex(GOD, (void *)m, tprintf("%ld", mech->mapindex));
  mech_Rsetxy(GOD, (void *)m, tprintf("%d %d", MechX(mech), MechY(mech)));
  mech_Rsetteam(GOD, (void *)m, tprintf("%d", MechTeam(mech)));
  move_via_teleport(evaluation, suit, mech->mapindex, 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);
  MechLOSBroadcast(m, tprintf("ejected from %s!", mech_display_id(mech).text));
  s_in_character(mech->xcode.context->database, suit);
  initialize_pc(player, m);
  silly_atr_set_in(m->xcode.context->database, m->mynum, A_PILOTNUM,
                   tprintf("#%ld", player));
  MechPilot(m) = player;
  MechTeam(m) = MechTeam(mech);
  /* MUDCONF THIS LATER (and to not copy digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT

  */
  m->freq[0] = random() % 1000000;
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                m->freq[0]);
  /* #endif
  #endif
  */
  notify(evaluation, player, "You eject from the unit!");
  if (MechType(mech) == CLASS_MECH) {
    DestroyPart(mech, HEAD, 2);
  }
  if (!Destroyed(mech)) {
    DestroyMech(mech, mech, 0, KILL_TYPE_EJECT);
  }
}

void mech_eject(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  cch(MECH_USUALS);
  DOCHECK_CONTEXT(mech->xcode.context, IsDS(mech),
                  "Dropships do not support ejection.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !((MechType(mech) == CLASS_MECH) ||
                    (MechType(mech) == CLASS_VTOL) ||
                    (MechType(mech) == CLASS_VEH_GROUND)),
                  "This unit has no ejection seat!");
  DOCHECK_CONTEXT(
      mech->xcode.context, FlyingT(mech) && !Landed(mech),
      "Regrettably, right now you can only eject when landed, sorry - no "
      "parachute :P");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !is_in_character(mech->xcode.context->database, mech->mynum),
                  "This unit isn't in character!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !mech->xcode.context->configuration->btech_ic,
                  "This MUX isn't in character!");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !is_in_character(
          mech->xcode.context->database,
          game_object_location(mech->xcode.context->database, mech->mynum)),
      "Your location isn't in character!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  Started(mech) && MechPilot(mech) != player,
                  "You aren't in da pilot's seat - no ejection for you!");
  if (!Started(mech)) {
    DOCHECK_CONTEXT(
        mech->xcode.context,
        (char_lookupplayer(
            mech->xcode.context, GOD, GOD, 0,
            btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                 A_PILOTNUM, (char[LBUF_SIZE]){0}))) != player,
        "You aren't the official pilot of this thing. Try 'disembark'");
  }
  if (MechType(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(
        mech->xcode.context, PartIsNonfunctional(mech, HEAD, 2),
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
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  suit = create_obj(
      evaluation, GOD, OBJECT_TYPE_THING,
      tprintf("MechWarrior - %s",
              game_object_name(mech->xcode.context->database, player)));
  silly_atr_set_in(mech->xcode.context->database, suit, A_XTYPE, "MECH");
  s_xcode(mech->xcode.context->database, suit);
  btech_special_object_flag_changed(mech->xcode.context, GOD, suit, 0, 1);
  d = btech_attribute_read(mech->xcode.context->database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech->xcode.context, suit))) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
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
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("Unable to load mechwarrior template for #%ld's "
                               "disembarkation. (%s)",
                               player, (!d || !*d) ? "Default template" : d));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't load MWTemplate)");
    return;
  }
  silly_atr_set_in(mech->xcode.context->database, suit, A_MECHNAME,
                   "MechWarrior");
  MechTeam(m) = MechTeam(mech);
  mech_Rsetmapindex(GOD, (void *)m, tprintf("%ld", mech->mapindex));
  mech_Rsetxy(GOD, (void *)m, tprintf("%d %d", MechX(mech), MechY(mech)));
  MechZ(m) = MechZ(mech);
  mech_Rsetteam(GOD, (void *)m, tprintf("%d", MechTeam(mech)));
  move_via_teleport(evaluation, suit, mech->mapindex, 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);
  s_in_character(mech->xcode.context->database, suit);
  initialize_pc(player, m);
  MechPilot(m) = player;
  silly_atr_set_in(m->xcode.context->database, m->mynum, A_PILOTNUM,
                   tprintf("#%ld", player));
  MechTeam(m) = MechTeam(mech);
  /* MUDCONF THIS LATER AND FIX (to not copy digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT
  */
  m->freq[0] = random() % 1000000;
  notify_printf(evaluation, player, "Emergency radio channel set to %d.",
                m->freq[0]);
  /* #endif
  #endif
  */
  mymap = btech_context_get_map(mech->xcode.context, m->mapindex);
  if ((MechZ(m) > (Elevation(mymap, MechX(m), MechY(m)) + 1)) &&
      (MechZ(m) > 0)) {
    notify(evaluation, player,
           "You open the hatch and climb out of the unit. Maybe you should "
           "have done this while the thing was closer to the ground...");
    MechLOSBroadcast(m, tprintf("jumps out of %s... in mid air !",
                                mech_display_id(mech).text));
    initial_speed = ((MechSpeed(mech) + MechVerticalSpeed(mech)) / MP1) / 2 + 4;
    mech_event_schedule(m, EVENT_FALL, mech_fall_event, FALL_TICK,
                        -initial_speed);
  } else {
    MechLOSBroadcast(m,
                     tprintf("climbs out of %s!", mech_display_id(mech).text));
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
      mech->xcode.context,
      !((MechType(mech) == CLASS_MECH) || (MechType(mech) == CLASS_VTOL) ||
        (MechType(mech) == CLASS_VEH_GROUND)),
      "The door ! The door ? The Door ?!? Where's the exit in this damned "
      "thing ?");

  /*  DOCHECK_CONTEXT(mech->xcode.context, FlyingT(mech) && !Landed(mech),
   * "What, in the air ? Are you suicidal ?"); */
  DOCHECK_CONTEXT(mech->xcode.context,
                  !is_in_character(mech->xcode.context->database, mech->mynum),
                  "This unit isn't in character!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !mech->xcode.context->configuration->btech_ic,
                  "This MUX isn't in character!");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !is_in_character(
          mech->xcode.context->database,
          game_object_location(mech->xcode.context->database, mech->mynum)),
      "Your location isn't in character!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (Started(mech) || mech_event_count(mech, EVENT_STARTUP)) &&
                      (MechPilot(mech) == player),
                  "While it's running!? Don't be daft.");
  DOCHECK_CONTEXT(mech->xcode.context, fabs(MechSpeed(mech)) > 25.,
                  "Are you suicidal ? That thing is moving too fast !");
  /* Ok.. time to disembark ourselves */
  char_disembark(player, mech);
}

/**
 * Handle the disembarking of units from within carriers.
 */
void mech_udisembark(DbRef player, void *data, char *buffer) {

  Mech *mech = (Mech *)data; /* The disembarking unit */
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  Mech *target;
  int newmech;       /* The carrier. */
  BattleMap *mymap;  /* The map to disembark to */
  int under_repairs; /* Is the unit still under repairs? */
  int i;             /* Used in section recycle for loop. */

  /* Any IN_CHARACTER unit's pilot must match the invoker to disembark.
   * A unit that is not IC can be disembarked by anyone.
   */
  DOCHECK_CONTEXT(
      mech->xcode.context,
      is_in_character(mech->xcode.context->database, mech->mynum) &&
          !is_wizard(mech->xcode.context->database, player) &&
          (char_lookupplayer(mech->xcode.context, GOD, GOD, 0,
                             btech_attribute_read(
                                 mech->xcode.context->database, mech->mynum,
                                 A_PILOTNUM, (char[LBUF_SIZE]){0})) != player),
      "This isn't your mech!");

  /* Find the carrier that the invoker's unit is in and check it for validity.
   */
  newmech = game_object_location(mech->xcode.context->database, mech->mynum);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(is_good_obj(mech->xcode.context->database, newmech) &&
                    is_xcode(mech->xcode.context->database, newmech)),
                  "You're not being carried!");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !(target = btech_context_get_mech(mech->xcode.context, newmech)),
      "Not being carried!");
  DOCHECK_CONTEXT(mech->xcode.context, target->mapindex == -1,
                  "You are not on a map.");

  /* Don't allow repairing units to disembark */
  under_repairs = figure_latest_tech_event(mech);
  DOCHECK_CONTEXT(
      mech->xcode.context, under_repairs,
      "This 'Mech is still under repairs (see checkstatus for more info)");

  DOCHECK_CONTEXT(
      mech->xcode.context,
      fabs(MechSpeed(target)) > WalkingSpeed(MMaxSpeed(target)),
      "You cannot leave while the carrier is moving faster than walk speed!");

  /* Carry out the disembarking. */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)target->mapindex));
  mech_Rsetxy(GOD, (void *)mech,
              tprintf("%d %d", MechX(target), MechY(target)));
  MechZ(mech) = MechZ(target);
  MechFZ(mech) = ZSCALE * MechZ(mech);
  mymap = btech_context_get_map(mech->xcode.context, mech->mapindex);
  DOCHECK_CONTEXT(mech->xcode.context, !mymap,
                  "Major map error possible. Prolly should contact a wizard.");

  /* Teleport loudly so native enter events and other messages run. */
  move_via_teleport(evaluation, mech->mynum, mech->mapindex, 1, 0);

  /* If we make it safely, start the invoker's unit up once it's on the map. */
  if (!Destroyed(mech) && game_object_location(mech->xcode.context->database,
                                               player) == mech->mynum) {
    MechPilot(mech) = player;
    mech_power_up(mech);
  }

  MarkForLOSUpdate(mech);
  SetCargoWeight(mech);
  UnSetMechPKiller(mech);
  MechLOSBroadcast(mech, "powers up!");
  EvalBit(
      MechSpecials(mech), SS_ABILITY,
      ((MechPilot(mech) > 0 &&
        is_player(mech->xcode.context->database, MechPilot(mech)))
           ? char_getvalue(mech->xcode.context, MechPilot(mech), "Sixth_Sense")
           : 0));
  MechComm(mech) = DEFAULT_COMM;

  if (is_player(mech->xcode.context->database, MechPilot(mech)) &&
      !is_quiet(mech->xcode.context->database, mech->mynum)) {
    MechComm(mech) = char_getskilltarget(mech->xcode.context, MechPilot(mech),
                                         "Comm-Conventional", 0);
    MechPer(mech) = char_getskilltarget(mech->xcode.context, MechPilot(mech),
                                        "Perception", 0);
  } else {
    MechComm(mech) = 6;
    MechPer(mech) = 6;
  }
  MechCommLast(mech) = 0;
  autopilot_resume_for_mech(mech);
  CargoSpace(target) += (MechTons(mech) * 100);
  MarkForLOSUpdate(target);

  /* A hidden carrier that is disembarked from loses its HIDDEN status */
  if (MechCritStatus(target) & HIDDEN) {
    MechCritStatus(target) &= ~HIDDEN;
    MechLOSBroadcast(target, "becomes visible as it is disembarked from.");
  }

  /* Para-dropping out of units from elevations. */
  if (!FlyingT(mech) &&
      MechZ(mech) > Elevation(mymap, MechX(mech), MechY(mech)) &&
      MechZ(mech) > 0) {

    notify(evaluation, player,
           "You open the hatch and drop out of the unit....");
    MechLOSBroadcast(
        mech, tprintf("drops out of %s and begins falling to the ground.",
                      mech_display_id(target).text));
    initiate_ood(player, mech,
                 tprintf("%d %d %d", MechX(mech), MechY(mech), MechZ(mech)));
  } else {
    if (MechType(mech) == CLASS_BSUIT) {
      MechLOSBroadcast(
          mech, tprintf("climbs out of %s!", mech_display_id(target).text));
      notify(evaluation, player, "You climb out of the unit.");
    } else {
      /* If the carrier is destroyed, do damage to the disembarking unit. */
      if (Destroyed(target) || !Started(target)) {
        MechLOSBroadcast(
            mech, tprintf("smashes open the ramp door and emerges from %s!",
                          mech_display_id(target).text));
        notify(evaluation, player, "You smash open the door and break out.");
        MechFalls(mech, 4, 0);
      } else {
        /* All is well. */
        MechLOSBroadcast(mech, tprintf("emerges from the ramp out of %s!",
                                       mech_display_id(target).text));
        notify(evaluation, player, "You emerge from the unit loading ramp.");
        if (Landed(mech) &&
            MechZ(mech) > Elevation(mymap, MechX(mech), MechY(mech)) &&
            FlyingT(mech))
          MechStatus(mech) &= ~LANDED;
      }
    }
  }

  /* Recycle any weapons/sections they have to prevent munchkin behavior. */
  if (MechType(mech) == CLASS_BSUIT) {
    StartBSuitRecycle(mech, 20);
  } else if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW) {
    for (i = 0; i < NUM_SECTIONS; i++)
      mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  } else if (MechType(mech) == CLASS_VEH_GROUND ||
             MechType(mech) == CLASS_VTOL) {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (i == ROTOR)
        continue;
      else
        mech_set_recycle_limb(mech, i, PHYSICAL_RECYCLE_TIME);
  }

  fix_pilotdamage(mech, MechPilot(mech));
  correct_speed(target);
} /* end mech_udisembark */
