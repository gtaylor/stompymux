#include "mech_physical_internal.h"
int all_limbs_recycled(Mech *mech) {
  if (MechSections(mech)[LARM].recycle || MechSections(mech)[RARM].recycle) {
    mech_notify(mech, MECHALL,
                "You still have arms recovering from another attack.");
    return 0;
  }

  if (MechSections(mech)[RLEG].recycle || MechSections(mech)[LLEG].recycle) {
    mech_notify(mech, MECHALL,
                "Your legs are still recovering from your last attack.");
    return 0;
  }
  // Fall through to success.
  return 1;
} // end all_limbs_recycled()

/**
 * Returns the correct verb for each physical attack.
 */
char *phys_form(int AttackType, int add_s) {
  // Holds our attack verb.
  char *verb;

  // See if we need the verb with an s on the end.
  if (add_s) {
    // With the S.
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
      // Ohboy, we're using some funky, unknown physical.
    default:
      verb = "??bugs??";
    } // end switch()
  } else {
    // Without the S.
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

#define phys_message(txt) MechLOSBroadcasti(mech, target, txt)

void phys_succeed(Mech *mech, Mech *target, int at) {
  phys_message(tprintf("%s %%s!", phys_form(at, 1)));
}

void phys_fail(Mech *mech, Mech *target, int at) {
  phys_message(tprintf("attempts to %s %%s!", phys_form(at, 0)));
}

/*
 * All 'mechs with arms can punch.
 */
int have_punch(Mech *mech, int loc) { return 1; }

/**
 * Does our unit have an axe?
 */
int have_axe(Mech *mech, int loc) {
  return FindObj(mech, loc, I2Special(AXE)) >= (MechTons(mech) / 15);
}

int have_claw(Mech *mech, int loc) {
  return FindObj(mech, loc, I2Special(CLAW)) >= (MechTons(mech) / 15);
}

/**
 * Does our unit have a dual_saw?
 */
int have_saw(Mech *mech, int loc) {
  return FindObj(mech, loc, I2Special(DUAL_SAW)) >= 7;
}

/**
 * Does our unit have a sword?
 */
int have_sword(Mech *mech, int loc) {
  return FindObj(mech, loc, I2Special(SWORD)) >= ((MechTons(mech) + 15) / 20);
}

/**
 * Does our unit have a mace?
 */
int have_mace(Mech *mech, int loc) {
  return FindObj(mech, loc, I2Special(MACE)) >= (MechTons(mech) / 10);
}

/*
 * Carry out some checks common to all types of physical attacks.
 */
int phys_common_checks(Mech *mech) {
  if (Jumping(mech)) {
    mech_notify(mech, MECHALL,
                "You can't perform physical attacks while in the air!");
    return 0;
  }

  if (mech_event_count(mech, EVENT_STAND)) {
    mech_notify(mech, MECHALL, "You are still trying to stand up!");
    return 0;
  }
#ifdef BT_MOVEMENT_MODES
  if (Dodging(mech) || mech_move_mode_locked(mech)) {
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

  // Fall through to success.
  return 0;
} // end get_arm_args()

/**
 * Performs some generic checks for arms to punch with.
 */
int punch_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == LARM ? "left" : "right");

  if (SectIsDestroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't punch with it.", arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 0, SHOULDER_OR_HIP)) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't punch with that arm.",
                arm_used);
    return 0;
  } else if (MechSections(mech)[arm].specials & CARRYING_CLUB) {
    mech_printf(
        mech, MECHALL,
        "You're carrying a club in your %s arm and can't punch with it.",
        arm_used);
    return 0;
  }
  // Fall through to success.
  return 1;
} // end checkArm()

/**
 * Mech punch routines.
 */
void mech_punch(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int punching = P_LEFT | P_RIGHT;

  // Carry out the common checks (started, on map, etc.)
  cch(MECH_USUALO);
  // Make sure we have arms to punch with.
  ARM_PHYS_CHECK("punch");
  // Disallow quads from punching.
  QUAD_CHECK("punch");

  argc = mech_parseattributes(buffer, args, 5);

  // If the directive is true, use the pilot's piloting skill. If not, we
  // use a constant BTH of 4.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *args[5];
  int argc;
  int clubLoc = -1;

  // Make sure unit is started, on map, etc.
  cch(MECH_USUALO);
  // Make sure we're in a biped.
  ARM_PHYS_CHECK("club");
  // Don't let quads club.
  QUAD_CHECK("club");

  if (MechSections(mech)[RARM].specials & CARRYING_CLUB)
    clubLoc = RARM;
  else if (MechSections(mech)[LARM].specials & CARRYING_CLUB)
    clubLoc = LARM;

  if (clubLoc == -1) {
    DOCHECKMA(mech_real_terrain_get(mech) != HEAVY_FOREST &&
                  mech_real_terrain_get(mech) != LIGHT_FOREST,
              "You can not seem to find any trees around to club with.");
    // Since we have trees nearby, assume the club goes to right hand.
    clubLoc = RARM;
  }

  argc = mech_parseattributes(buffer, args, 5);

  DOCHECKMA(SectIsDestroyed(mech, LARM),
            "Your left arm is destroyed, you can't club.");
  DOCHECKMA(SectIsDestroyed(mech, RARM),
            "Your right arm is destroyed, you can't club.");
  DOCHECKMA(
      !OkayCritSectS(RARM, 0, SHOULDER_OR_HIP),
      "You can't club anyone with a destroyed or missing right shoulder.");
  DOCHECKMA(!OkayCritSectS(LARM, 0, SHOULDER_OR_HIP),
            "You can't club anyone with a destroyed or missing left shoulder.");
  DOCHECKMA(!OkayCritSectS(RARM, 3, HAND_OR_FOOT_ACTUATOR),
            "You can't club anyone with a destroyed or missing right hand.");
  DOCHECKMA(!OkayCritSectS(LARM, 3, HAND_OR_FOOT_ACTUATOR),
            "You can't club anyone with a destroyed or missing left hand.");

  // Clubbing is usually done with the right arm but a club may be
  // grabbed by the left hand. Clubbing requires both arms to be cycled,
  // but only one is checked by PhysicalAttack(). So, we check both
  // here just in case.
  DOCHECKMA(SectHasBusyWeap(mech, LARM) || SectHasBusyWeap(mech, RARM),
            "You have weapons recycling on your arms.");

  PhysicalAttack(mech, 5,
                 (mech->xcode.context->configuration->btech_phys_use_pskill
                      ? FindPilotPiloting(mech) - 1
                      : 4),
                 PA_CLUB, argc, args, mech_map, RARM);
} // end mech_club()

/**
 * Check to see if the specified arm can be used to axe with.
 */
int axe_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (SectIsDestroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't axe with it", arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 0, SHOULDER_OR_HIP)) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't axe with that arm.",
                arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 3, HAND_OR_FOOT_ACTUATOR)) {
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);
  // Do we have arms?
  ARM_PHYS_CHECK("axe");
  // Make sure we're not a quad.
  QUAD_CHECK("axe");

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
  DOCHECKMA(!using, "You may lack the axe, but not the will! Try punch/club "
                    "until you find one.");
} // end mech_axe()

/**
 * Check to see if the specified arm can be used to saw with.
 */
int saw_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (SectIsDestroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't saw with it", arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 0, SHOULDER_OR_HIP)) {
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);
  // Do we have arms?
  ARM_PHYS_CHECK("saw");
  // Make sure we're not a quad.
  QUAD_CHECK("saw");

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
  DOCHECKMA(!using, "You don't have a dual saw!");
} // end mech_saw()

/**
 * Mech punch routines.
 */
void mech_claw(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Carry out the common checks (started, on map, etc.)
  cch(MECH_USUALO);
  // Make sure we have arms to claw with.
  ARM_PHYS_CHECK("claw");
  // Disallow quads from clawing.
  QUAD_CHECK("claw");

  argc = mech_parseattributes(buffer, args, 5);

  // If the directive is true, use the pilot's piloting skill. If not, we
  // use a constant BTH of 4.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
  DOCHECKMA(!using,
            "You do not have any claws! Try punching/clubbing instead!");

} // end mech_claw()

/**
 * Check our arms to see if they can mace.
 */
int mace_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (SectIsDestroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't use a mace with it.",
                arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 0, SHOULDER_OR_HIP)) {
    mech_printf(
        mech, MECHALL,
        "Your %s shoulder is destroyed, you can't use a mace with that arm.",
        arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 3, HAND_OR_FOOT_ACTUATOR)) {
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 4, rtohit = 4;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);
  // Do we have arms?
  ARM_PHYS_CHECK("mace");
  // Make sure we're not a quad.
  QUAD_CHECK("mace");

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is on, use the player's piloting skill.
  // If not, assume a skill level of 4.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
  DOCHECKMA(!using, "You don't have a mace!");
} // end mech_mace()

/**
 * Check our arms to see if they can chop.
 */
int sword_checkArm(Mech *mech, int arm) {
  char *arm_used = (arm == RARM ? "right" : "left");

  if (SectIsDestroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't use a sword with it.",
                arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 0, SHOULDER_OR_HIP)) {
    mech_printf(
        mech, MECHALL,
        "Your %s shoulder is destroyed, you can't use a sword with that arm.",
        arm_used);
    return 0;
  } else if (!OkayCritSectS(arm, 3, HAND_OR_FOOT_ACTUATOR)) {
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
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *argl[5];
  char **args = argl;
  int argc, ltohit = 3, rtohit = 3;
  int using = P_LEFT | P_RIGHT;

  // Make sure we're started, on a map, etc.
  cch(MECH_USUALO);
  // Do we have arms to chop with?
  ARM_PHYS_CHECK("chop");
  // Quads can't do it.
  QUAD_CHECK("chop");

  argc = mech_parseattributes(buffer, args, 5);

  // If btech_phys_use_pskill is defined, use the pilot's piloting skill,
  // otherwise use a constant skill 3.
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
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
  DOCHECKMA(!using, "You have no sword to chop people with!");
} // end mech_sword()

/**
 * Mech tripping command hook.
 */
