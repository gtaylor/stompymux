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
#include "eject_api.h"
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

void mech_embark(DbRef player, void *data, char *buffer) {

  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  Mech *target, *towee = NULL;
  int tmp;
  DbRef target_num;
  int argc;
  char *args[4];
  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  if (player != GOD)
    cch(MECH_USUAL);
  if (MechType(mech) == CLASS_MW) {
    argc = mech_parseattributes(buffer, args, 1);
    DOCHECK_CONTEXT(mech->xcode.context, argc != 1,
                    "Invalid number of arguements.");
    target_num = FindTargetDBREFFromMapNumber(mech, args[0]);
    DOCHECK_CONTEXT(mech->xcode.context, target_num == -1,
                    "That target is not in your line of sight.");
    target = btech_context_get_mech(mech->xcode.context, target_num);
    DOCHECK_CONTEXT(mech->xcode.context,
                    !target || !mech_los_check(mech, target, MechX(target),
                                               MechY(target),
                                               FaMechRange(mech, target)),
                    "That target is not in your line of sight.");
    DOCHECK_CONTEXT(mech->xcode.context, OODing(target),
                    "You should wait for your target to land first");
    DOCHECK_CONTEXT(mech->xcode.context, MechZ(mech) > (MechZ(target) + 1),
                    "You are too high above the target.");
    DOCHECK_CONTEXT(mech->xcode.context, MechZ(mech) < (MechZ(target) - 1),
                    "You can't reach that high !");
    DOCHECK_CONTEXT(mech->xcode.context,
                    MechX(mech) != MechX(target) ||
                        MechY(mech) != MechY(target),
                    "You need to be in the same hex!");
    DOCHECK_CONTEXT(
        mech->xcode.context,
        (!is_in_character(mech->xcode.context->database, mech->mynum)) ||
            (!is_in_character(mech->xcode.context->database, target->mynum)),
        "You don't really see a way to get in there.");
    DOCHECK_CONTEXT(
        mech->xcode.context,
        (MechType(target) == CLASS_VEH_GROUND ||
         MechType(target) == CLASS_VTOL) &&
            !unit_is_fixable(target),
        "You can't find and entrance amid the mass of twisted metal.");

    if (!lock_test(evaluation, player, player, mech->mynum, target->mynum,
                   LUA_LOCK_ENTER, LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock,
                   &lock_result)) {

      /* Trigger FAIL & AFAIL */
      memset(fail_mesg, 0, sizeof(fail_mesg));
      snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

      notify_lock_failure(evaluation, &lock, &lock_result, fail_mesg, NULL,
                          LUA_EVENT_FAIL);

      return;
    }

    if (!lock_result.defined) {

      /* Check their teams */
      DOCHECK_CONTEXT(mech->xcode.context, MechTeam(mech) != MechTeam(target),
                      "Locked. Damn !");
    }

    DOCHECK_CONTEXT(mech->xcode.context, fabs(MechSpeed(target)) > 15.,
                    "Are you suicidal ? That thing is moving too fast !");

    if (MechType(target) == CLASS_MECH) {
      DOCHECK_CONTEXT(
          mech->xcode.context, !GetSectInt(target, HEAD),
          "Okay, just climb up to-- Wait... where did the head go??");
      DOCHECK_CONTEXT(mech->xcode.context, PartIsDestroyed(target, HEAD, 2),
                      "Okay, just climb up and open-- "
                      "WTF ? Someone stole the cockpit!");
      DOCHECK_CONTEXT(
          mech->xcode.context, PartIsNonfunctional(target, HEAD, 2),
          "Okay, just climb up and open-- hey, this door won't budge!");
    }
    mech_notify(mech, MECHALL,
                tprintf("You climb into %s.", mech_display_id(target).text));
    mech_los_broadcast(
        mech, tprintf("climbs into %s.", mech_display_id(target).text));
    tele_contents(mech->xcode.context, mech->mynum, target->mynum, TELE_ALL);
    discard_mw(mech);
    return;
  }
  /* What heppens with a Bsuit squad? */
  /* Check if the vechile has cargo capacity, or is an Omni Mech */
  argc = mech_parseattributes(buffer, args, 1);
  DOCHECK_CONTEXT(mech->xcode.context, argc != 1,
                  "Invalid number of arguements.");
  target_num = FindTargetDBREFFromMapNumber(mech, args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, target_num == -1,
                  "That target is not in your line of sight.");
  target = btech_context_get_mech(mech->xcode.context, target_num);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !target ||
                      !mech_los_check(mech, target, MechX(target),
                                      MechY(target), FaMechRange(mech, target)),
                  "That target is not in your line of sight.");
  DOCHECK_CONTEXT(mech->xcode.context, MechCarrying(mech) == target_num,
                  "You cannot embark what your towing!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  Fallen(mech) || mech_event_count(mech, EVENT_STAND),
                  "Help! I've fallen and I can't get up!");
  DOCHECK_CONTEXT(mech->xcode.context, !Started(mech) || Destroyed(mech),
                  "Ha Ha Ha.");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You cannot do that while jumping!");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(target),
                  "You cannot do that while it is jumping!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechSpecials2(mech) & CARRIER_TECH &&
                      (IsDS(target) ? IsDS(mech) : 1),
                  "You're a bit bulky to do that yourself.");
  DOCHECK_CONTEXT(mech->xcode.context, MechCritStatus(mech) & HIDDEN,
                  "You cannot embark while hidden.");
  DOCHECK_CONTEXT(mech->xcode.context, MechTons(mech) > CarMaxTon(target),
                  "You are too large for that class of carrier.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_BSUIT &&
                      !(MechSpecials2(target) & CARRIER_TECH),
                  "This unit can't handle your mass.");
  DOCHECK_CONTEXT(mech->xcode.context, MMaxSpeed(mech) < MP1,
                  "You are to overloaded to enter.");
  DOCHECK_CONTEXT(mech->xcode.context, MechZ(mech) > (MechZ(target) + 1),
                  "You are too high above the target.");
  DOCHECK_CONTEXT(mech->xcode.context, MechZ(mech) < (MechZ(target) - 1),
                  "You can't reach that high !");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechX(mech) != MechX(target) || MechY(mech) != MechY(target),
                  "You need to be in the same hex!");

  if (!lock_test(evaluation, player, player, mech->mynum, target->mynum,
                 LUA_LOCK_ENTER, LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock,
                 &lock_result)) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

    notify_lock_failure(evaluation, &lock, &lock_result, fail_mesg, NULL,
                        LUA_EVENT_FAIL);

    return;
  }

  if (!lock_result.defined) {

    /* Check their teams */
    DOCHECK_CONTEXT(mech->xcode.context, MechTeam(mech) != MechTeam(target),
                    "Locked. Damn !");
  }

  DOCHECK_CONTEXT(mech->xcode.context, fabs(MechSpeed(target)) > 0,
                  "Are you suicidal ? That thing is moving too fast !");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !is_in_character(mech->xcode.context->database, mech->mynum) ||
          !is_in_character(mech->xcode.context->database, target->mynum),
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

  DOCHECK_CONTEXT(mech->xcode.context,
                  (MechTons(mech) * 100) > CargoSpace(target),
                  "Not enough cargospace for you!");
  if (MechCarrying(mech) > 0) {
    DOCHECK_CONTEXT(mech->xcode.context,
                    !(towee = btech_context_get_mech(mech->xcode.context,
                                                     MechCarrying(mech))),
                    "Internal error caused by towed unit! Contact a wizard!");
    DOCHECK_CONTEXT(mech->xcode.context, MechTons(towee) > CarMaxTon(target),
                    "Your towed unit is  too large for that class of carrier.");
    DOCHECK_CONTEXT(mech->xcode.context,
                    ((MechTons(mech) + MechTons(towee)) * 100) >
                        CargoSpace(target),
                    "Not enough cargospace for you and your towed unit!");
  }
  if (MechType(mech) == CLASS_BSUIT) {
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
    if (towee && MechCarrying(mech) > 0) {
      mech_notify(towee, MECHALL,
                  tprintf("You are drug up the entry ramp into %s.",
                          mech_display_id(target).text));
      mech_los_broadcast(towee, tprintf("is drug up the entry ramp into %s.",
                                        mech_display_id(target).text));
    }
  }
  MarkForLOSUpdate(mech);
  MarkForLOSUpdate(target);

  if (MechCritStatus(target) & HIDDEN) {
    MechCritStatus(target) &= ~HIDDEN;
    mech_los_broadcast(target, "becomes visible as it is embarked into.");
  }

  /* Check if the unit is towing something so the towed unit
   * is handled first because mech_power_down() will cause it to drop
   * whatever its towing */
  if (towee && MechCarrying(mech) > 0) {
    MarkForLOSUpdate(towee);
    mech_Rsetmapindex(GOD, (void *)towee, tprintf("%d", (int)-1));
    mech_Rsetxy(GOD, (void *)towee, tprintf("%d %d", 0, 0));
    move_via_teleport(evaluation, towee->mynum, target->mynum, 1, 0);
    CargoSpace(target) -= (MechTons(towee) * 100);
    mech_power_down(towee);
    SetCarrying(mech, -1);
    MechStatus(towee) &= ~TOWED;
  }

  /* Now handle the unit itself */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)-1));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", 0, 0));
  move_via_teleport(evaluation, mech->mynum, target->mynum, 1, 0);
  CargoSpace(target) -= (MechTons(mech) * 100);
  mech_power_down(mech);

  correct_speed(target);
}

void autoeject(DbRef player, Mech *mech, int tIsBSuit) {
  Mech *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  /* If we're not IC, return */
  if (!player || !is_in_character(mech->xcode.context->database, mech->mynum) ||
      !mech->xcode.context->configuration->btech_ic ||
      !is_in_character(
          mech->xcode.context->database,
          game_object_location(mech->xcode.context->database, mech->mynum)))
    return;

  /* Create the MW object */
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

  /* Tele the MW to the map and player to the MW */
  move_via_teleport(evaluation, suit, mech->mapindex, 1, 7);
  move_via_teleport(evaluation, player, suit, 1, 7);

  /* Init the sucker */
  s_in_character(mech->xcode.context->database, suit);
  initialize_pc(player, m);
  MechPilot(m) = player;
  MechTeam(m) = MechTeam(mech);
  /* MUDCONF THIS LATER (and fix not copying digital)
  #ifdef COPY_CHANS_ON_EJECT
          memcpy(m->freq, mech->freq, FREQS * sizeof(m->freq[0]));
          memcpy(m->freqmodes, mech->freqmodes, FREQS *
  sizeof(m->freqmodes[0])); #else #ifdef RANDOM_CHAN_ON_EJECT
  */
  m->freq[0] = random() % 1000000;
  notify(evaluation, player,
         tprintf("Emergency radio channel set to %d.", m->freq[0]));
  /* #endif
  #endif
  */

  if (tIsBSuit) {
    mech_los_broadcast(m, "climbs out of one of the destroyed suits!");
    notify(evaluation, player, "You climb out of the unit!");
  } else {
    mech_los_broadcast(m,
                       tprintf("ejected from %s!", mech_display_id(mech).text));
    mech_ood_initiate(player, m, tprintf("%d %d %d", MechX(m), MechY(m), 150));
    notify(evaluation, player, "You eject from the unit!");
  }
}
