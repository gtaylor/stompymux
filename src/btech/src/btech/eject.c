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

/* Ejection code */
#include "autopilot.h"
#include "glue.h"
#include "mech.events.h"
#include "mech.h"
#include "p.bsuit.h"
#include "p.btechstats.h"
#include "p.crit.h"
#include "p.econ_cmds.h"
#include "p.mech.combat.h"
#include "p.mech.combat.misc.h"
#include "p.mech.los.h"
#include "p.mech.ood.h"
#include "p.mech.pickup.h"
#include "p.mech.restrict.h"
#include "p.mech.tag.h"
#include "p.mech.tech.commands.h"
#include "p.mech.tech.h"
#include "p.mech.update.h"
#include "p.mech.utils.h"
#include "p.mechrep.h"
#include <math.h>

int tele_contents(BtechContext *context, DbRef from, DbRef to, int flag) {
  DbRef i, tmpnext;
  int count = 0;
  EvaluationContext *evaluation = btech_context_evaluation(context);

  SAFE_DOLIST(context->database, i, tmpnext,
              game_object_contents(context->database, from))
  if ((flag & TELE_ALL) || !Wiz(context->database, i)) {
    if (flag & TELE_XP && !Wiz(context->database, i))
      if (!(is_quiet(context->database, from)))
        lower_xp(context, i, context->configuration->btech_xploss);
    move_via_teleport(evaluation, i, to, 1, flag & TELE_LOUD ? 0 : 7);
    count++;
  }
  return count;
}

/* Delayed blast event, for various reasons */
static void mech_discard_event(MuxEvent *e) {
  MECH *mech = (MECH *)e->data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  DbRef i = mech->mynum;

  /* We'll silently move the mech off, but lets trigger the aleave/oleave/leave
   * of the loc as well before we do anything fancy */
  did_it(evaluation, i, game_object_location(mech->xcode.context->database, i),
         A_LEAVE, NULL, A_OLEAVE, NULL, A_ALEAVE, (char **)NULL, 0);
  c_hardcode(mech->xcode.context->database, i);
  handle_xcode(mech->xcode.context, GOD, i, 1, 0);
  s_going(mech->xcode.context->database, i);
  s_dark(mech->xcode.context->database, i);
  s_zombie(mech->xcode.context->database, i);
  move_via_teleport(evaluation, i,
                    mech->xcode.context->configuration->btech_usedmechstore, 1,
                    7);
}

void discard_mw(MECH *mech) {
  if (is_in_character(mech->xcode.context->database, mech->mynum))
    MECHEVENT(mech, EVENT_NUKEMECH, mech_discard_event, 10, 0);
}

void enter_mw_bay(MECH *mech, DbRef bay) {
  tele_contents(mech->xcode.context, mech->mynum, bay,
                TELE_ALL); /* Even immortals must get going */
  discard_mw(mech);
}

void pickup_mw(MECH *mech, MECH *target) {
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

static void char_eject(DbRef player, MECH *mech) {
  MECH *m;
  DbRef suit;
  char *d;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  suit = create_obj(
      evaluation, GOD, TYPE_THING,
      tprintf("MechWarrior - %s",
              game_object_name(mech->xcode.context->database, player)));
  silly_atr_set_in(mech->xcode.context->database, suit, A_XTYPE, "MECH");
  s_hardcode(mech->xcode.context->database, suit);
  handle_xcode(mech->xcode.context, GOD, suit, 0, 1);
  d = btech_attribute_read(mech->xcode.context->database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech->xcode.context, suit))) {
    SendError(
        mech->xcode.context,
        tprintf("Unable to create special obj for #%ld's ejection.", player));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't create RS object)");
    return;
  }
  if (!mech_loadnew(GOD, m,
                    (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    SendError(
        mech->xcode.context,
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
  MECH *mech = (MECH *)data;

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

static void char_disembark(DbRef player, MECH *mech) {
  MECH *m;
  DbRef suit;
  char *d;
  MAP *mymap;
  long initial_speed;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  suit = create_obj(
      evaluation, GOD, TYPE_THING,
      tprintf("MechWarrior - %s",
              game_object_name(mech->xcode.context->database, player)));
  silly_atr_set_in(mech->xcode.context->database, suit, A_XTYPE, "MECH");
  s_hardcode(mech->xcode.context->database, suit);
  s_opaque(mech->xcode.context->database, suit);
  handle_xcode(mech->xcode.context, GOD, suit, 0, 1);
  d = btech_attribute_read(mech->xcode.context->database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech->xcode.context, suit))) {
    SendError(mech->xcode.context,
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
    SendError(mech->xcode.context,
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
    MECHEVENT(m, EVENT_FALL, mech_fall_event, FALL_TICK, -initial_speed);
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
  MECH *mech = (MECH *)data;

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
                  (Started(mech) || Starting(mech)) &&
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

  MECH *mech = (MECH *)data; /* The disembarking unit */
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  MECH *target;
  int newmech;       /* The carrier. */
  MAP *mymap;        /* The map to disembark to */
  int under_repairs; /* Is the unit still under repairs? */
  int i;             /* Used in section recycle for loop. */

  /* Any IN_CHARACTER unit's pilot must match the invoker to disembark.
   * A unit that is not IC can be disembarked by anyone.
   */
  DOCHECK_CONTEXT(
      mech->xcode.context,
      is_in_character(mech->xcode.context->database, mech->mynum) &&
          !Wiz(mech->xcode.context->database, player) &&
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
                    is_hardcode(mech->xcode.context->database, newmech)),
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

  /* Teleporting loudly in order to trigger @aenter's and whatnot. */
  move_via_teleport(evaluation, mech->mynum, mech->mapindex, 1, 0);

  /* If we make it safely, start the invoker's unit up once it's on the map. */
  if (!Destroyed(mech) && game_object_location(mech->xcode.context->database,
                                               player) == mech->mynum) {
    MechPilot(mech) = player;
    Startup(mech);
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
  UnZombifyMech(mech);
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
      SetRecycleLimb(mech, i, PHYSICAL_RECYCLE_TIME);
  } else if (MechType(mech) == CLASS_VEH_GROUND ||
             MechType(mech) == CLASS_VTOL) {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (i == ROTOR)
        continue;
      else
        SetRecycleLimb(mech, i, PHYSICAL_RECYCLE_TIME);
  }

  fix_pilotdamage(mech, MechPilot(mech));
  correct_speed(target);
} /* end mech_udisembark */

void mech_embark(DbRef player, void *data, char *buffer) {

  MECH *mech = (MECH *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  MECH *target, *towee = NULL;
  int tmp;
  DbRef target_num;
  int argc;
  char *args[4];
  char fail_mesg[SBUF_SIZE];
  char enter_lock[LBUF_SIZE];
  long i, j;

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
                    !target || !InLineOfSight(mech, target, MechX(target),
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

    if (!could_doit_with_context(evaluation, mech->mynum, target->mynum,
                                 A_LENTER)) {

      /* Trigger FAIL & AFAIL */
      memset(fail_mesg, 0, sizeof(fail_mesg));
      snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

      did_it(evaluation, player, target->mynum, A_FAIL, fail_mesg, 0, NULL,
             A_AFAIL, (char **)NULL, 0);

      return;
    }

    /* They passed the lock but does that mean there was no lock? */
    memset(enter_lock, 0, sizeof(enter_lock));
    attribute_get_string(mech->xcode.context->database, enter_lock,
                         target->mynum, A_LENTER, &i, &j);
    if (*enter_lock == '\0') {

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
    MechLOSBroadcast(mech,
                     tprintf("climbs into %s.", mech_display_id(target).text));
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
                      !InLineOfSight(mech, target, MechX(target), MechY(target),
                                     FaMechRange(mech, target)),
                  "That target is not in your line of sight.");
  DOCHECK_CONTEXT(mech->xcode.context, MechCarrying(mech) == target_num,
                  "You cannot embark what your towing!");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech) || Standing(mech),
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

  if (!could_doit_with_context(evaluation, mech->mynum, target->mynum,
                               A_LENTER)) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    snprintf(fail_mesg, SBUF_SIZE, "That unit's bay doors are locked.");

    did_it(evaluation, player, target->mynum, A_FAIL, fail_mesg, 0, NULL,
           A_AFAIL, (char **)NULL, 0);

    return;
  }

  /* They passed the lock but does that mean there was no lock? */
  memset(enter_lock, 0, sizeof(enter_lock));
  attribute_get_string(mech->xcode.context->database, enter_lock, target->mynum,
                       A_LENTER, &i, &j);
  if (*enter_lock == '\0') {

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
  if ((tmp = MechFullNoRecycle(mech, CHECK_BOTH))) {

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
    MechLOSBroadcast(mech,
                     tprintf("climbs into %s.", mech_display_id(target).text));
  } else {
    mech_notify(mech, MECHALL,
                tprintf("You climb up the entry ramp into %s.",
                        mech_display_id(target).text));
    MechLOSBroadcast(mech, tprintf("climbs up the entry ramp into %s.",
                                   mech_display_id(target).text));
    if (towee && MechCarrying(mech) > 0) {
      mech_notify(towee, MECHALL,
                  tprintf("You are drug up the entry ramp into %s.",
                          mech_display_id(target).text));
      MechLOSBroadcast(towee, tprintf("is drug up the entry ramp into %s.",
                                      mech_display_id(target).text));
    }
  }
  MarkForLOSUpdate(mech);
  MarkForLOSUpdate(target);

  if (MechCritStatus(target) & HIDDEN) {
    MechCritStatus(target) &= ~HIDDEN;
    MechLOSBroadcast(target, "becomes visible as it is embarked into.");
  }

  /* Check if the unit is towing something so the towed unit
   * is handled first because Shutdown() will cause it to drop
   * whatever its towing */
  if (towee && MechCarrying(mech) > 0) {
    MarkForLOSUpdate(towee);
    mech_Rsetmapindex(GOD, (void *)towee, tprintf("%d", (int)-1));
    mech_Rsetxy(GOD, (void *)towee, tprintf("%d %d", 0, 0));
    move_via_teleport(evaluation, towee->mynum, target->mynum, 1, 0);
    CargoSpace(target) -= (MechTons(towee) * 100);
    Shutdown(towee);
    SetCarrying(mech, -1);
    MechStatus(towee) &= ~TOWED;
  }

  /* Now handle the unit itself */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)-1));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", 0, 0));
  move_via_teleport(evaluation, mech->mynum, target->mynum, 1, 0);
  CargoSpace(target) -= (MechTons(mech) * 100);
  Shutdown(mech);

  correct_speed(target);
}

void autoeject(DbRef player, MECH *mech, int tIsBSuit) {
  MECH *m;
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
      evaluation, GOD, TYPE_THING,
      tprintf("MechWarrior - %s",
              game_object_name(mech->xcode.context->database, player)));
  silly_atr_set_in(mech->xcode.context->database, suit, A_XTYPE, "MECH");
  s_hardcode(mech->xcode.context->database, suit);
  handle_xcode(mech->xcode.context, GOD, suit, 0, 1);
  d = btech_attribute_read(mech->xcode.context->database, player, A_MWTEMPLATE,
                           (char[LBUF_SIZE]){0});
  if (!(m = btech_context_get_mech(mech->xcode.context, suit))) {
    SendError(
        mech->xcode.context,
        tprintf("Unable to create special obj for #%ld's ejection.", player));
    destroy_thing(evaluation, suit);
    notify(evaluation, player,
           "Sorry, something serious went wrong, contact a Wizard "
           "(can't create RS object)");
    return;
  }
  if (!mech_loadnew(GOD, m,
                    (!d || !*d || !strcmp(d, "#-1")) ? "MechWarrior" : d)) {
    SendError(
        mech->xcode.context,
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
    MechLOSBroadcast(m, "climbs out of one of the destroyed suits!");
    notify(evaluation, player, "You climb out of the unit!");
  } else {
    MechLOSBroadcast(m,
                     tprintf("ejected from %s!", mech_display_id(mech).text));
    initiate_ood(player, m, tprintf("%d %d %d", MechX(m), MechY(m), 150));
    notify(evaluation, player, "You eject from the unit!");
  }
}
