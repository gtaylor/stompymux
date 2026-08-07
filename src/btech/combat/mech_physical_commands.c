#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_internal.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "registry_api.h"
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
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int rl = RLEG, ll = LLEG;
  int leg;
  int using = P_RIGHT;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // If we're a quad, re-map front legs.
  if (mech_is_quad(mech)) {
    rl = RARM;
    ll = LARM;
  }
  // See if we have enough usable legs to kick/trip with.
  int destroyed_legs = CountDestroyedLegs(mech);
  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot kick without a 'mech!");
    return;
  }
  if (mech_class(mech) != CLASS_MECH) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot kick with this vehicle!");
    return;
  }
  if (!mech_is_quad(mech) && (destroyed_legs > 1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Without legs? Are you kidding?");
    return;
  }
  if (!mech_is_quad(mech) && (destroyed_legs > 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "With one leg? Are you kidding?");
    return;
  }
  if (destroyed_legs > 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "It'd unbalance you too much in your condition..");
    return;
  }
  if (destroyed_legs > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Exactly _what_ are you going to kick with?");
    return;
  }

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

  if (mech_condition_summary(mech).hip_damaged) {
    mech_printf(mech, MECHALL, "You can't %s with a destroyed hip.",
                phys_form(AttackType, 0));
    return;
  }

  PhysicalAttack(
      mech, 5,
      (btech_context_physical_attacks_use_pilot_skill(mech_context(mech))
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
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int targetnum;
  char targetID[5];
  char *args[5];
  int argc;
  int wcDeadLegs = 0;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  // Mechwarriors can't chage.
  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot charge without a 'mech!");
    return;
  }

  // Salvage vehicles can't charge.
  if (mech_class(mech) != CLASS_MECH &&
      (mech_class(mech) != CLASS_VEH_GROUND ||
       mech_technology_flags(mech) & SALVAGE_TECH)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot charge with this vehicle!");
    return;
  }

  // Figure out if we have enough legs to kick with.
  if (mech_class(mech) == CLASS_MECH) {
    /* set the number of dead legs we have */
    wcDeadLegs = CountDestroyedLegs(mech);

    if (!mech_is_quad(mech) && (wcDeadLegs > 0)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "With one leg? Are you kidding?");
      return;
    }
    if (!mech_is_quad(mech) && (wcDeadLegs > 1)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Without legs? Are you kidding?");
      return;
    }
    if (wcDeadLegs > 1) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "It'd unbalance you too much in your condition..");
      return;
    }
    if (wcDeadLegs > 2) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Exactly _what_ are you going to kick with?");
      return;
    }
  } // end if() - Dead leg counting.

  argc = mech_parseattributes(buffer, args, 2);

  if (mech_event_count(mech, EVENT_MOVEMODE)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot charge while changing movement modes!");
    return;
  }

  if (mech_condition_summary(mech).sprinting ||
      mech_condition_summary(mech).evading) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot charge while in a special movement mode!");
    return;
  }
  if (mech_condition_summary(mech).dodging) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot charge while dodging!");
    return;
  }

  switch (argc) {
    // No arguments given with charge. Assume default target.
  case 0:
    if (mech_target_dbref(mech) == -1) {
      mech_notify(mech, MECHALL, "You do not have a default target set!");
      return;
    }

    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));

    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      mech_targeting_target_clear(mech);
      return;
    }
    // Don't allow charging Mechwarriors.
    if (mech_class(target) == CLASS_MW) {
      mech_notify(mech, MECHALL,
                  "You can't charge THAT sack of bones and squishy bits!");
      return;
    }

    if (battle_map_blocks_friendly_fire(mech_map) &&
        (mech_team(mech) == mech_team(target))) {
      mech_notify(mech, MECHALL, "You can't charge your own team!");
      mech_charge_target_dbref_set(mech, -1);
      return;
    }

    mech_charge_target_dbref_set(mech, mech_target_dbref(mech));
    mech_notify(mech, MECHALL, "Charge target set to default target.");
    break;

    // We've supplied an argument, either a '-' or an ID.
  case 1:
    if (args[0][0] == '-') {
      mech_charge_reset(mech);
      mech_notify(mech, MECHPILOT, "You are no longer charging.");
      return;
    }

    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);

    if (targetnum == -1) {
      mech_notify(mech, MECHALL, "Target is not in line of sight!");
      return;
    }

    target = btech_context_get_mech(mech_context(mech), targetnum);
    if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                  mech_position_y(target),
                                  mech_range_to(mech, target))) {
      mech_notify(mech, MECHALL, "Target is not in line of sight!");
      return;
    }

    if (!target) {
      mech_notify(mech, MECHALL, "Invalid target data!");
      return;
    }

    if (battle_map_blocks_friendly_fire(mech_map) &&
        (mech_team(mech) == mech_team(target))) {
      mech_notify(mech, MECHALL, "You can't charge your own team!");
      mech_charge_target_dbref_set(mech, -1);
      return;
    }

    // Don't allow charging mechwarriors.
    if (mech_class(target) == CLASS_MW) {
      mech_notify(mech, MECHALL,
                  "You can't charge THAT sack of bones and squishy bits!");
      return;
    }

    mech_charge_target_dbref_set(mech, targetnum);

    mech_printf(mech, MECHALL, "%s target set to %s.",
                mech_class(mech) == CLASS_MECH ? "Charge" : "Ram",
                mech_to_mech_display_id(mech, target).text);
    break;

    // Something other than 0-1 arguments.
  default:
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
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
