#include "mech_physical_internal.h"
void mech_trip(DbRef player, void *data, char *buffer) {
  mech_kickortrip(player, data, buffer, PA_TRIP);
} // end mech_trip()

/**
 * Mech kick command hook.
 */
void mech_kick(DbRef player, void *data, char *buffer) {
  mech_kickortrip(player, data, buffer, PA_KICK);
} // end mech_trip()

/**
 * Mech kick/trip routines.
 */
void mech_kickortrip(DbRef player, void *data, char *buffer, int AttackType) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc;
  int rl = RLEG, ll = LLEG;
  int leg;
  int using = P_RIGHT;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);
  // If we're a quad, re-map front legs.
  if (MechIsQuad(mech)) {
    rl = RARM;
    ll = LARM;
  }
  // See if we have enough usable legs to kick/trip with.
  GENERIC_CHECK("kick", CountDestroyedLegs(mech));

  argc = mech_parseattributes(buffer, args, 5);

  // Figure out which leg we're using.
  if (get_arm_args(&using, &argc, &args, mech, have_punch, "")) {
    return;
  }

  switch (using) {
  case P_LEFT:
    leg = ll;
    break;

  case P_RIGHT:
    leg = rl;
    break;

  default:
  case P_LEFT | P_RIGHT:
    mech_notify(mech, MECHALL, "What, yer gonna LEVITATE? I Don't Think So.");
    return;
  }

  if ((MechCritStatus(mech) & HIP_DAMAGED)) {
    mech_printf(mech, MECHALL, "You can't %s with a destroyed hip.",
                phys_form(AttackType, 0));
    return;
  }

  PhysicalAttack(mech, 5,
                 (mech->xcode.context->configuration->btech_phys_use_pskill
                      ? FindPilotPiloting(mech) - 2
                      : 3),
                 AttackType, argc, args, mech_map, leg);
} // end mech_kickortrip()

/**
 * Mech/tank charge routines
 */
void mech_charge(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  int targetnum;
  char targetID[5];
  char *args[5];
  int argc;
  int wcDeadLegs = 0;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);

  // Mechwarriors can't chage.
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_MW || MechType(mech) == CLASS_BSUIT,
                  "You cannot charge without a 'mech!");

  // Salvage vehicles can't charge.
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_MECH &&
                      (MechType(mech) != CLASS_VEH_GROUND ||
                       MechSpecials(mech) & SALVAGE_TECH),
                  "You cannot charge with this vehicle!");

  // Figure out if we have enough legs to kick with.
  if (MechType(mech) == CLASS_MECH) {
    /* set the number of dead legs we have */
    wcDeadLegs = CountDestroyedLegs(mech);

    DOCHECK_CONTEXT(mech->xcode.context, !MechIsQuad(mech) && (wcDeadLegs > 0),
                    "With one leg? Are you kidding?");
    DOCHECK_CONTEXT(mech->xcode.context, !MechIsQuad(mech) && (wcDeadLegs > 1),
                    "Without legs? Are you kidding?");
    DOCHECK_CONTEXT(mech->xcode.context, wcDeadLegs > 1,
                    "It'd unbalance you too much in your condition..");
    DOCHECK_CONTEXT(mech->xcode.context, wcDeadLegs > 2,
                    "Exactly _what_ are you going to kick with?");
  } // end if() - Dead leg counting.

  argc = mech_parseattributes(buffer, args, 2);

  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You cannot charge while changing movement modes!");

  DOCHECK_CONTEXT(mech->xcode.context, Sprinting(mech) || Evading(mech),
                  "You cannot charge while in a special movement mode!");
  DOCHECK_CONTEXT(mech->xcode.context, Dodging(mech),
                  "You cannot charge while dodging!");

  switch (argc) {
    // No arguments given with charge. Assume default target.
  case 0:
    DOCHECKMA(MechTarget(mech) == -1, "You do not have a default target set!");

    target = btech_context_get_mech(mech->xcode.context, MechTarget(mech));

    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      MechTarget(mech) = -1;
      return;
    }
    // Don't allow charging Mechwarriors.
    if (MechType(target) == CLASS_MW) {
      mech_notify(mech, MECHALL,
                  "You can't charge THAT sack of bones and squishy bits!");
      return;
    }

    if (MapNoFriendlyFire(mech_map) && (MechTeam(mech) == MechTeam(target))) {
      mech_notify(mech, MECHALL, "You can't charge your own team!");
      MechChargeTarget(mech) = -1;
      return;
    }

    MechChargeTarget(mech) = MechTarget(mech);
    mech_notify(mech, MECHALL, "Charge target set to default target.");
    break;

    // We've supplied an argument, either a '-' or an ID.
  case 1:
    if (args[0][0] == '-') {
      MechChargeTarget(mech) = -1;
      MechChargeTimer(mech) = 0;
      MechChargeDistance(mech) = 0;
      mech_notify(mech, MECHPILOT, "You are no longer charging.");
      return;
    }

    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);

    DOCHECKMA(targetnum == -1, "Target is not in line of sight!");

    target = btech_context_get_mech(mech->xcode.context, targetnum);
    DOCHECKMA(!InLineOfSight_NB(mech, target, MechX(target), MechY(target),
                                FaMechRange(mech, target)),
              "Target is not in line of sight!");

    if (!target) {
      mech_notify(mech, MECHALL, "Invalid target data!");
      return;
    }

    if (MapNoFriendlyFire(mech_map) && (MechTeam(mech) == MechTeam(target))) {
      mech_notify(mech, MECHALL, "You can't charge your own team!");
      MechChargeTarget(mech) = -1;
      return;
    }

    // Don't allow charging mechwarriors.
    if (MechType(target) == CLASS_MW) {
      mech_notify(mech, MECHALL,
                  "You can't charge THAT sack of bones and squishy bits!");
      return;
    }

    MechChargeTarget(mech) = targetnum;

    mech_printf(mech, MECHALL, "%s target set to %s.",
                MechType(mech) == CLASS_MECH ? "Charge" : "Ram",
                mech_to_mech_display_id(mech, target).text);
    break;

    // Something other than 0-1 arguments.
  default:
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Invalid number of arguments!");
  }
} // end mech_charge()

/*
 * Home to the code to carry out physical attacks.
 *
 * NOTE: Do NOT put any logic/checker code in here that is specific to a
 * certain type of attack if possible. Put it in its respective function.
 * Only things that are generic and on-specific to an attack type go here.
 */
