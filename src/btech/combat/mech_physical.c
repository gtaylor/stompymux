#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_internal.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "registry_api.h"

static bool physical_arm_check(DbRef player, Mech *mech, const char *verb) {
  BtechContext *context = mech_context(mech);

  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("You cannot %s without a 'mech!", verb));
    return false;
  }
  if (mech_class(mech) != CLASS_MECH) {
    mecha_notify(btech_context_evaluation(context), player,
                 tprintf("You cannot %s with this vehicle!", verb));
    return false;
  }
  return true;
}

static bool physical_quad_check(DbRef player, Mech *mech, const char *verb) {
  if (mech_class(mech) != CLASS_MECH || !mech_is_quad(mech))
    return true;

  mecha_notify(
      btech_context_evaluation(mech_context(mech)), player,
      tprintf("What are you going to %s with, your front right leg?", verb));
  return false;
}

int all_limbs_recycled(Mech *mech) {
  if (mech_section_recycle_ticks(mech, LARM) ||
      mech_section_recycle_ticks(mech, RARM)) {
    mech_notify(mech, MECHALL,
                "You still have arms recovering from another attack.");
    return 0;
  }

  if (mech_section_recycle_ticks(mech, RLEG) ||
      mech_section_recycle_ticks(mech, LLEG)) {
    mech_notify(mech, MECHALL,
                "Your legs are still recovering from your last attack.");
    return 0;
  }
  return 1;
} // end all_limbs_recycled()

char *phys_form(PhysicalAttackType AttackType, int add_s) {
  char *verb;

  if (add_s) {
    switch (AttackType) {
    case PA_PUNCH:
      verb = "punchs";
      break;
    case PA_CLUB:
      verb = "clubs";
      break;
    case PA_MACE:
      verb = "maces";
      break;
    case PA_SWORD:
      verb = "chops";
      break;
    case PA_AXE:
      verb = "axes";
      break;
    case PA_KICK:
      verb = "kicks";
      break;
    case PA_TRIP:
      verb = "trips";
      break;
    case PA_SAW:
      verb = "saws";
      break;
    case PA_CLAW:
      verb = "claws";
      break;
    default:
      verb = "??bugs??";
    } // end switch()
  } else {
    switch (AttackType) {
    case PA_PUNCH:
      verb = "punch";
      break;
    case PA_CLUB:
      verb = "club";
      break;
    case PA_MACE:
      verb = "maces";
      break;
    case PA_SWORD:
      verb = "chop";
      break;
    case PA_AXE:
      verb = "axe";
      break;
    case PA_KICK:
      verb = "kick";
      break;
    case PA_TRIP:
      verb = "trip";
      break;
    case PA_SAW:
      verb = "saw";
      break;
    case PA_CLAW:
      verb = "claw";
      break;
      // Ohboy, we're using some funky, unknown physical.
    default:
      verb = "??bugs??";
    } // end switch()
  } // end if/else()

  return verb;
} // end phys_form

void phys_succeed(Mech *mech, Mech *target, PhysicalAttackType at) {
  mech_los_broadcast_unit(mech, target, tprintf("%s %%s!", phys_form(at, 1)));
}

void phys_fail(Mech *mech, Mech *target, PhysicalAttackType at) {
  mech_los_broadcast_unit(mech, target,
                          tprintf("attempts to %s %%s!", phys_form(at, 0)));
}

int have_punch(Mech *mech, int loc) { return 1; }

/**
 * Does our unit have an axe?
 */
int have_axe(Mech *mech, int loc) {
  return FindObj(mech, loc, special_equipment_index(AXE)) >=
         (mech_tonnage(mech) / 15);
}

int have_claw(Mech *mech, int loc) {
  return FindObj(mech, loc, special_equipment_index(CLAW)) >=
         (mech_tonnage(mech) / 15);
}

/**
 * Does our unit have a dual_saw?
 */
int have_saw(Mech *mech, int loc) {
  return FindObj(mech, loc, special_equipment_index(DUAL_SAW)) >= 7;
}

/**
 * Does our unit have a sword?
 */
int have_sword(Mech *mech, int loc) {
  return FindObj(mech, loc, special_equipment_index(SWORD)) >=
         ((mech_tonnage(mech) + 15) / 20);
}

/**
 * Does our unit have a mace?
 */
int have_mace(Mech *mech, int loc) {
  return FindObj(mech, loc, special_equipment_index(MACE)) >=
         (mech_tonnage(mech) / 10);
}

/*
 * Carry out some checks common to all types of physical attacks.
 */
int phys_common_checks(Mech *mech) {
  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL,
                "You can't perform physical attacks while in the air!");
    return 0;
  }

  if (mech_event_count(mech, EVENT_STAND)) {
    mech_notify(mech, MECHALL, "You are still trying to stand up!");
    return 0;
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_condition_summary(mech).dodging || mech_move_mode_locked(mech)) {
    mech_notify(
        mech, MECHALL,
        "You cannot use physicals while using a special movement mode.");
    return 0;
  }
#endif

  if (!all_limbs_recycled(mech)) {
    return 0;
  }
  // Fall through to success.
  return 1;
} // end phys_common_checks()

/*
 * Parse a physical attack command's arguments that allow an arm or both
 * to be specified. eg. AXE [B|L|R] [ID]
 */
int get_arm_args(int *using, int *argc, char ***args, Mech *mech,
                 int (*have_fn)(Mech *mech, int loc), char *weapon) {

  if (*argc != 0 && args[0][0][0] != '\0' && args[0][0][1] == '\0') {
    char arm = toupper(args[0][0][0]);

    // Determine which flag we're dealing with (Both, Left, Right)
    switch (arm) {
    case 'B':
      *using = P_LEFT | P_RIGHT;
      --*argc;
      ++*args;
      break;

    case 'L':
      *using = P_LEFT;
      --*argc;
      ++*args;
      break;

    case 'R':
      *using = P_RIGHT;
      --*argc;
      ++*args;
    } // end switch()
  } // end if()

  // Check for the presence of specified arms, or pick one. *using set in
  // the above switch statement.
  switch (*using) {
  case P_LEFT:
    if (!have_fn(mech, LARM)) {
      mech_printf(mech, MECHALL, "You don't have %s in your left arm!", weapon);
      return 1;
    }
    break;

  case P_RIGHT:
    if (!have_fn(mech, RARM)) {
      mech_printf(mech, MECHALL, "You don't have %s in your right arm!",
                  weapon);
      return 1;
    }
    break;

  case P_LEFT | P_RIGHT:
    if (!have_fn(mech, LARM))
      *using &= ~P_LEFT;
    if (!have_fn(mech, RARM))
      *using &= ~P_RIGHT;
    break;
  } // end switch()

  return 0;
} // end get_arm_args()

/**
 * Performs some generic checks for arms to punch with.
 */
int punch_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == LARM ? "left" : "right");

  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't punch with it.", arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 0,
                                                   SHOULDER_OR_HIP)) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't punch with that arm.",
                arm_used);
    return 0;
  } else if (mech_section_carries_club(mech, arm)) {
    mech_printf(
        mech, MECHALL,
        "You're carrying a club in your %s arm and can't punch with it.",
        arm_used);
    return 0;
  }
  return 1;
} // end checkArm()

/**
 * Mech punch routines.
 */
void mech_punch(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int punching = P_LEFT | P_RIGHT;

  // Carry out the common checks (started, on map, etc.)
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Make sure we have arms to punch with.
  if (!physical_arm_check(player, mech, "punch"))
    return;
  // Disallow quads from punching.
  if (!physical_quad_check(player, mech, "punch"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If the directive is true, use the pilot's piloting skill. If not, we
  // use a constant BTH of 4.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    rtohit = ltohit = FindPilotPiloting(mech);

  // Manipulate punching var to contain only the arms we're punching with.
  if (get_arm_args(&punching, &argc, &args, mech, have_punch, "")) {
    return;
  }
  // Carry out our standard physical checks. This happens in PhysicalAttack
  // but the player gets double-spammed since PhysicalAttack can be called
  // twice in here. So, we add the check before.
  if (!phys_common_checks(mech))
    return;

  // For each arm we're using, check to make sure it's good to punch with
  // and carry out the roll if it is.
  if (punching & P_LEFT) {
    if (punch_checkArm(mech, LARM))
      PhysicalAttack(mech, 10, ltohit, PA_PUNCH, argc, args, mech_map, LARM);
  }

  if (punching & P_RIGHT) {
    if (punch_checkArm(mech, RARM))
      PhysicalAttack(mech, 10, rtohit, PA_PUNCH, argc, args, mech_map, RARM);
  }
} // end mech_punch()

/**
 * Mech clubbing routines.
 */
void mech_club(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *args[5];
  int argc;
  int clubLoc = -1;

  // Make sure unit is started, on map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Make sure we're in a biped.
  if (!physical_arm_check(player, mech, "club"))
    return;
  // Don't let quads club.
  if (!physical_quad_check(player, mech, "club"))
    return;

  if (mech_section_carries_club(mech, RARM))
    clubLoc = RARM;
  else if (mech_section_carries_club(mech, LARM))
    clubLoc = LARM;

  if (clubLoc == -1) {
    if (mech_real_terrain_get(mech) != HEAVY_FOREST &&
        mech_real_terrain_get(mech) != LIGHT_FOREST) {
      mech_notify(mech, MECHALL,
                  "You can not seem to find any trees around to club with.");
      return;
    }
    // Since we have trees nearby, assume the club goes to right hand.
    clubLoc = RARM;
  }

  argc = mech_parseattributes(buffer, args, 5);

  if (mech_section_is_destroyed(mech, LARM)) {
    mech_notify(mech, MECHALL, "Your left arm is destroyed, you can't club.");
    return;
  }
  if (mech_section_is_destroyed(mech, RARM)) {
    mech_notify(mech, MECHALL, "Your right arm is destroyed, you can't club.");
    return;
  }
  if (!mech_critical_is_operational_special(mech, RARM, 0, SHOULDER_OR_HIP)) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing right shoulder.");
    return;
  }
  if (!mech_critical_is_operational_special(mech, LARM, 0, SHOULDER_OR_HIP)) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing left shoulder.");
    return;
  }
  if (!mech_critical_is_operational_special(mech, RARM, 3,
                                            HAND_OR_FOOT_ACTUATOR)) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing right hand.");
    return;
  }
  if (!mech_critical_is_operational_special(mech, LARM, 3,
                                            HAND_OR_FOOT_ACTUATOR)) {
    mech_notify(mech, MECHALL,
                "You can't club anyone with a destroyed or missing left hand.");
    return;
  }

  // Clubbing is usually done with the right arm but a club may be
  // grabbed by the left hand. Clubbing requires both arms to be cycled,
  // but only one is checked by PhysicalAttack(). So, we check both
  // here just in case.
  if (mech_section_has_recycling_weapon(mech, LARM) ||
      mech_section_has_recycling_weapon(mech, RARM)) {
    mech_notify(mech, MECHALL, "You have weapons recycling on your arms.");
    return;
  }

  PhysicalAttack(
      mech, 5,
      (btech_context_physical_attacks_use_pilot_skill(mech_context(mech))
           ? FindPilotPiloting(mech) - 1
           : 4),
      PA_CLUB, argc, args, mech_map, RARM);
} // end mech_club()

/**
 * Check to see if the specified arm can be used to axe with.
 */
int axe_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't axe with it", arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 0,
                                                   SHOULDER_OR_HIP)) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't axe with that arm.",
                arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 3,
                                                   HAND_OR_FOOT_ACTUATOR)) {
    mech_printf(mech, MECHALL,
                "Your %s hand is destroyed, you can't axe with that arm.",
                arm_used);
    return 0;
  }
  // Fall through to success.
  return 1;
} // end axe_checkArm()

/**
 * Mech axe routines.
 */
void mech_axe(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Do we have arms?
  if (!physical_arm_check(player, mech, "axe"))
    return;
  // Make sure we're not a quad.
  if (!physical_quad_check(player, mech, "axe"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = FindPilotPiloting(mech) - 1;

  // Figure out which arm to use.
  if (get_arm_args(&using, &argc, &args, mech, have_axe, "an axe")) {
    return;
  }

  if (using & P_LEFT) {
    if (axe_checkArm(mech, LARM))
      PhysicalAttack(mech, 5, ltohit, PA_AXE, argc, args, mech_map, LARM);
  }
  if (using & P_RIGHT) {
    if (axe_checkArm(mech, RARM))
      PhysicalAttack(mech, 5, rtohit, PA_AXE, argc, args, mech_map, RARM);
  }
  // We don't have an axe.
  if (!using) {
    mech_notify(mech, MECHALL,
                "You may lack the axe, but not the will! Try punch/club "
                "until you find one.");
    return;
  }
} // end mech_axe()

/**
 * Check to see if the specified arm can be used to saw with.
 */
int saw_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't saw with it", arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 0,
                                                   SHOULDER_OR_HIP)) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't saw with that arm.",
                arm_used);
    return 0;
  }
  // Fall through to success.
  return 1;
} // end saw_checkArm()

/**
 * Mech dual saw routines.
 */
void mech_saw(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Do we have arms?
  if (!physical_arm_check(player, mech, "saw"))
    return;
  // Make sure we're not a quad.
  if (!physical_quad_check(player, mech, "saw"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = FindPilotPiloting(mech) - 1;

  // Figure out which arm to use.
  if (get_arm_args(&using, &argc, &args, mech, have_saw, "a saw")) {
    return;
  }

  if (using & P_LEFT) {
    if (saw_checkArm(mech, LARM))
      PhysicalAttack(mech, 7, ltohit, PA_SAW, argc, args, mech_map, LARM);
  }
  if (using & P_RIGHT) {
    if (saw_checkArm(mech, RARM))
      PhysicalAttack(mech, 7, rtohit, PA_SAW, argc, args, mech_map, RARM);
  }
  // We don't have a saw.
  if (!using) {
    mech_notify(mech, MECHALL, "You don't have a dual saw!");
    return;
  }
} // end mech_saw()

/**
 * Mech punch routines.
 */
void mech_claw(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Carry out the common checks (started, on map, etc.)
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Make sure we have arms to claw with.
  if (!physical_arm_check(player, mech, "claw"))
    return;
  // Disallow quads from clawing.
  if (!physical_quad_check(player, mech, "claw"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If the directive is true, use the pilot's piloting skill. If not, we
  // use a constant BTH of 4.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    rtohit = ltohit = FindPilotPiloting(mech);

  // Manipulate punching var to contain only the arms we're punching with.
  if (get_arm_args(&using, &argc, &args, mech, have_claw, "a claw")) {
    return;
  }
  // Carry out our standard physical checks. This happens in PhysicalAttack
  // but the player gets double-spammed since PhysicalAttack can be called
  // twice in here. So, we add the check before.
  if (!phys_common_checks(mech))
    return;

  // For each arm we're using, check to make sure it's good to punch with
  // and carry out the roll if it is.
  if (using & P_LEFT) {
    PhysicalAttack(mech, 7, ltohit, PA_CLAW, argc, args, mech_map, LARM);
  }

  if (using & P_RIGHT) {
    PhysicalAttack(mech, 7, rtohit, PA_CLAW, argc, args, mech_map, RARM);
  }

  // We don't have a claw
  if (!using) {
    mech_notify(mech, MECHALL,
                "You do not have any claws! Try punching/clubbing instead!");
    return;
  }

} // end mech_claw()

/**
 * Check our arms to see if they can mace.
 */
int mace_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't use a mace with it.",
                arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 0,
                                                   SHOULDER_OR_HIP)) {
    mech_printf(
        mech, MECHALL,
        "Your %s shoulder is destroyed, you can't use a mace with that arm.",
        arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 3,
                                                   HAND_OR_FOOT_ACTUATOR)) {
    mech_printf(
        mech, MECHALL,
        "Your %s hand is destroyed, you can't use a mace with that arm.",
        arm_used);
    return 0;
  }
  // Fall through to success.
  return 1;
} // end mace_checkArm()

/**
 * Mech mace routines.
 */
void mech_mace(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Do we have arms?
  if (!physical_arm_check(player, mech, "mace"))
    return;
  // Make sure we're not a quad.
  if (!physical_quad_check(player, mech, "mace"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = FindPilotPiloting(mech) - 1;

  // Figure out which arm to use.
  if (get_arm_args(&using, &argc, &args, mech, have_mace, "a mace")) {
    return;
  }

  if (using & P_LEFT) {
    if (mace_checkArm(mech, LARM))
      PhysicalAttack(mech, 4, ltohit, PA_MACE, argc, args, mech_map, LARM);
  }
  if (using & P_RIGHT) {
    if (mace_checkArm(mech, RARM))
      PhysicalAttack(mech, 4, rtohit, PA_MACE, argc, args, mech_map, RARM);
  }
  // We don't have a mace.
  if (!using) {
    mech_notify(mech, MECHALL, "You don't have a mace!");
    return;
  }
} // end mech_mace()

/**
 * Check our arms to see if they can chop.
 */
int sword_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't use a sword with it.",
                arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 0,
                                                   SHOULDER_OR_HIP)) {
    mech_printf(
        mech, MECHALL,
        "Your %s shoulder is destroyed, you can't use a sword with that arm.",
        arm_used);
    return 0;
  } else if (!mech_critical_is_operational_special(mech, arm, 3,
                                                   HAND_OR_FOOT_ACTUATOR)) {
    mech_printf(
        mech, MECHALL,
        "Your %s hand is destroyed, you can't use a sword with that arm.",
        arm_used);
    return 0;
  }
  // Fall through to success.
  return 1;
} // end sword_checkArm()

/**
 * Mech sword routines.
 */
void mech_sword(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 3, rtohit = 3;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  // Do we have arms to chop with?
  if (!physical_arm_check(player, mech, "chop"))
    return;
  // Quads can't do it.
  if (!physical_quad_check(player, mech, "chop"))
    return;

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is defined, use the pilot's piloting skill,
  // otherwise use a constant skill 3.
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = FindPilotPiloting(mech) - 2;

  // Which arm(s) have sword crits?
  if (get_arm_args(&using, &argc, &args, mech, have_sword, "a sword")) {
    return;
  }

  if (using & P_LEFT) {
    if (sword_checkArm(mech, LARM))
      PhysicalAttack(mech, 10, ltohit, PA_SWORD, argc, args, mech_map, LARM);
  }

  if (using & P_RIGHT) {
    if (sword_checkArm(mech, RARM))
      PhysicalAttack(mech, 10, rtohit, PA_SWORD, argc, args, mech_map, RARM);
  }
  // Ninja what?
  if (!using) {
    mech_notify(mech, MECHALL, "You have no sword to chop people with!");
    return;
  }
} // end mech_sword()

/**
 * Mech tripping command hook.
 */
