#include "mech_physical.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_physical_internal.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include <stddef.h>
#include <stdio.h>
#include <string.h>
bool physical_arm_check(DbRef player, Mech *mech, const char *verb) {
  BtechContext *context = mech_context(mech);
  if (mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT) {
    mecha_notifyf(btech_context_evaluation(context), player,
                  "You cannot %s without a 'mech!", verb);
    return false;
  }
  if (mech_class(mech) != CLASS_MECH) {
    mecha_notifyf(btech_context_evaluation(context), player,
                  "You cannot %s with this vehicle!", verb);
    return false;
  }
  return true;
}
bool physical_quad_check(DbRef player, Mech *mech, const char *verb) {
  if (mech_class(mech) != CLASS_MECH || !mech_is_quad(mech))
    return true;
  mecha_notifyf(btech_context_evaluation(mech_context(mech)), player,
                "What are you going to %s with, your front right leg?", verb);
  return false;
}
static bool all_limbs_recycled(Mech *mech) {
  if (mech_section_recycle_ticks(mech, LARM) ||
      mech_section_recycle_ticks(mech, RARM)) {
    mech_notify(mech, MECHALL,
                "You still have arms recovering from another attack.");
    return false;
  }
  if (mech_section_recycle_ticks(mech, RLEG) ||
      mech_section_recycle_ticks(mech, LLEG)) {
    mech_notify(mech, MECHALL,
                "Your legs are still recovering from your last attack.");
    return false;
  }
  return true;
} // end all_limbs_recycled()
const char *physical_attack_verb(const PhysicalVerbRequest *request) {
  const PhysicalAttackType ATTACK_TYPE = request->attack_type;
  const char *verb;
  if (request->third_person) {
    switch (ATTACK_TYPE) {
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
    switch (ATTACK_TYPE) {
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
    default:
      verb = "??bugs??";
    } // end switch()
  } // end if/else()
  return verb;
} // end phys_form
void phys_succeed(Mech *mech, Mech *target, PhysicalAttackType at) {
  char message_buffer[LBUF_SIZE];
  (void)snprintf(message_buffer, sizeof(message_buffer), "%s %%s!",
                 physical_attack_verb(&(PhysicalVerbRequest){
                     .attack_type = at, .third_person = true}));
  mech_los_broadcast_unit(mech, target, message_buffer);
}
void phys_fail(Mech *mech, Mech *target, PhysicalAttackType at) {
  char message_buffer[LBUF_SIZE];
  (void)snprintf(
      message_buffer, sizeof(message_buffer), "attempts to %s %%s!",
      physical_attack_verb(&(PhysicalVerbRequest){.attack_type = at}));
  mech_los_broadcast_unit(mech, target, message_buffer);
}
bool have_punch(Mech *mech [[maybe_unused]], int loc [[maybe_unused]]) {
  return true;
}
bool have_axe(Mech *mech, int loc) {
  return find_obj(mech, loc, special_equipment_index(AXE)) >=
         (mech_tonnage(mech) / 15);
}
bool have_claw(Mech *mech, int loc) {
  return find_obj(mech, loc, special_equipment_index(CLAW)) >=
         (mech_tonnage(mech) / 15);
}
bool have_saw(Mech *mech, int loc) {
  return find_obj(mech, loc, special_equipment_index(DUAL_SAW)) >= 7;
}
bool have_sword(Mech *mech, int loc) {
  return find_obj(mech, loc, special_equipment_index(SWORD)) >=
         ((mech_tonnage(mech) + 15) / 20);
}
bool have_mace(Mech *mech, int loc) {
  return find_obj(mech, loc, special_equipment_index(MACE)) >=
         (mech_tonnage(mech) / 10);
}
bool phys_common_checks(Mech *mech) {
  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL,
                "You can't perform physical attacks while in the air!");
    return false;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mech_notify(mech, MECHALL, "You are still trying to stand up!");
    return false;
  }
  if (!all_limbs_recycled(mech)) {
    return false;
  }
  return true;
} // end phys_common_checks()
ArmSelectionResult physical_arm_select(const ArmSelectionRequest *request) {
  int using = request->using;
  int argc = request->argument_count;
  char **args = request->arguments;
  Mech *mech = request->mech;
  PhysicalEquipmentCheck have_fn = request->has_weapon;
  const char *weapon = request->weapon;
  if (argc != 0) {
    char **first_slot = (char **)checked_storage_at((void *)args, (size_t)argc,
                                                    sizeof(*args), 0);
    const char *first = *first_slot;
    if (strlen(first) != 1)
      goto arm_selection_complete;
    const int ARM =
        *first >= 'a' && *first <= 'z' ? *first - 'a' + 'A' : *first;
    switch (ARM) {
    case 'B':
      using = P_LEFT | P_RIGHT;
      --argc;
      if (argc > 0)
        args = (char **)checked_storage_at((void *)args, (size_t)argc + 1,
                                           sizeof(*args), 1);
      break;
    case 'L':
      using = P_LEFT;
      --argc;
      if (argc > 0)
        args = (char **)checked_storage_at((void *)args, (size_t)argc + 1,
                                           sizeof(*args), 1);
      break;
    case 'R':
      using = P_RIGHT;
      --argc;
      if (argc > 0)
        args = (char **)checked_storage_at((void *)args, (size_t)argc + 1,
                                           sizeof(*args), 1);
    } // end switch()
  } // end if()
arm_selection_complete:
  switch (using) {
  case P_LEFT:
    if (!have_fn(mech, LARM)) {
      mech_printf(mech, MECHALL, "You don't have %s in your left arm!", weapon);
      return (ArmSelectionResult){.failed = true};
    }
    break;
  case P_RIGHT:
    if (!have_fn(mech, RARM)) {
      mech_printf(mech, MECHALL, "You don't have %s in your right arm!",
                  weapon);
      return (ArmSelectionResult){.failed = true};
    }
    break;
  case P_LEFT | P_RIGHT:
    if (!have_fn(mech, LARM))
      using &= ~P_LEFT;
    if (!have_fn(mech, RARM))
      using &= ~P_RIGHT;
    break;
  } // end switch()
  return (ArmSelectionResult){
      .using = using, .argument_count = argc, .arguments = args};
} // end get_arm_args()
static bool punch_check_arm(Mech *mech, int arm) {
  const char *arm_used = (arm == LARM ? "left" : "right");
  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't punch with it.", arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't punch with that arm.",
                arm_used);
    return false;
  }
  if (mech_section_carries_club(mech, arm)) {
    mech_printf(
        mech, MECHALL,
        "You're carrying a club in your %s arm and can't punch with it.",
        arm_used);
    return false;
  }
  return true;
} // end checkArm()
void mech_punch(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int ltohit = 4;
  int rtohit = 4;
  int punching = P_LEFT | P_RIGHT;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "punch"))
    return;
  if (!physical_quad_check(player, mech, "punch"))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    rtohit = ltohit = find_pilot_piloting(mech);
  ArmSelectionResult selection = physical_arm_select(&(ArmSelectionRequest){
      .using = punching,
      .argument_count = argc,
      .arguments = args,
      .mech = mech,
      .has_weapon = have_punch,
      .weapon = "",
  });
  if (selection.failed) {
    return;
  }
  punching = selection.using;
  argc = selection.argument_count;
  args = selection.arguments;
  if (!phys_common_checks(mech))
    return;
  if (punching & P_LEFT) {
    if (punch_check_arm(mech, LARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 10,
                                                       .base_to_hit = ltohit,
                                                       .attack_type = PA_PUNCH,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = LARM});
    }
  }
  if (punching & P_RIGHT) {
    if (punch_check_arm(mech, RARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 10,
                                                       .base_to_hit = rtohit,
                                                       .attack_type = PA_PUNCH,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = RARM});
    }
  }
} // end mech_punch()
void mech_club(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *args[5];
  int argc;
  int club_loc = -1;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "club"))
    return;
  if (!physical_quad_check(player, mech, "club"))
    return;
  if (mech_section_carries_club(mech, RARM))
    club_loc = RARM;
  else if (mech_section_carries_club(mech, LARM))
    club_loc = LARM;
  if (club_loc == -1) {
    if (mech_real_terrain_get(mech) != HEAVY_FOREST &&
        mech_real_terrain_get(mech) != LIGHT_FOREST) {
      mech_notify(mech, MECHALL,
                  "You can not seem to find any trees around to club with.");
      return;
    }
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
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = RARM, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing right shoulder.");
    return;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = LARM, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing left shoulder.");
    return;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = RARM, .critical = 3},
                                  .special = HAND_OR_FOOT_ACTUATOR})) {
    mech_notify(
        mech, MECHALL,
        "You can't club anyone with a destroyed or missing right hand.");
    return;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = LARM, .critical = 3},
                                  .special = HAND_OR_FOOT_ACTUATOR})) {
    mech_notify(mech, MECHALL,
                "You can't club anyone with a destroyed or missing left hand.");
    return;
  }
  if (mech_section_has_recycling_weapon(mech, LARM) ||
      mech_section_has_recycling_weapon(mech, RARM)) {
    mech_notify(mech, MECHALL, "You have weapons recycling on your arms.");
    return;
  }
  physical_attack_resolve(&(PhysicalAttackRequest){
      .mech = mech,
      .damage_weight = 5,
      .base_to_hit =
          btech_context_physical_attacks_use_pilot_skill(mech_context(mech))
              ? find_pilot_piloting(mech) - 1
              : 4,
      .attack_type = PA_CLUB,
      .argument_count = argc,
      .arguments = args,
      .map = mech_map,
      .section = RARM});
} // end mech_club()
static bool axe_check_arm(Mech *mech, int arm) {
  const char *arm_used = (arm == RARM ? "right" : "left");
  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't axe with it", arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't axe with that arm.",
                arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 3},
                                  .special = HAND_OR_FOOT_ACTUATOR})) {
    mech_printf(mech, MECHALL,
                "Your %s hand is destroyed, you can't axe with that arm.",
                arm_used);
    return false;
  }
  return true;
} // end axe_checkArm()
void mech_axe(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int ltohit = 4;
  int rtohit = 4;
  int using = P_LEFT | P_RIGHT;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "axe"))
    return;
  if (!physical_quad_check(player, mech, "axe"))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = find_pilot_piloting(mech) - 1;
  ArmSelectionResult selection = physical_arm_select(&(ArmSelectionRequest){
      .using = using,
      .argument_count = argc,
      .arguments = args,
      .mech = mech,
      .has_weapon = have_axe,
      .weapon = "an axe",
  });
  if (selection.failed) {
    return;
  }
  using = selection.using;
  argc = selection.argument_count;
  args = selection.arguments;
  if (using & P_LEFT) {
    if (axe_check_arm(mech, LARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 5,
                                                       .base_to_hit = ltohit,
                                                       .attack_type = PA_AXE,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = LARM});
    }
  }
  if (using & P_RIGHT) {
    if (axe_check_arm(mech, RARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 5,
                                                       .base_to_hit = rtohit,
                                                       .attack_type = PA_AXE,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = RARM});
    }
  }
  if (!using) {
    mech_notify(mech, MECHALL,
                "You may lack the axe, but not the will! Try punch/club "
                "until you find one.");
    return;
  }
} // end mech_axe()
static bool saw_check_arm(Mech *mech, int arm) {
  const char *arm_used = (arm == RARM ? "right" : "left");
  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't saw with it", arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_printf(mech, MECHALL,
                "Your %s shoulder is destroyed, you can't saw with that arm.",
                arm_used);
    return false;
  }
  return true;
} // end saw_checkArm()
void mech_saw(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int ltohit = 4;
  int rtohit = 4;
  int using = P_LEFT | P_RIGHT;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "saw"))
    return;
  if (!physical_quad_check(player, mech, "saw"))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = find_pilot_piloting(mech) - 1;
  ArmSelectionResult selection = physical_arm_select(&(ArmSelectionRequest){
      .using = using,
      .argument_count = argc,
      .arguments = args,
      .mech = mech,
      .has_weapon = have_saw,
      .weapon = "a saw",
  });
  if (selection.failed) {
    return;
  }
  using = selection.using;
  argc = selection.argument_count;
  args = selection.arguments;
  if (using & P_LEFT) {
    if (saw_check_arm(mech, LARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 7,
                                                       .base_to_hit = ltohit,
                                                       .attack_type = PA_SAW,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = LARM});
    }
  }
  if (using & P_RIGHT) {
    if (saw_check_arm(mech, RARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 7,
                                                       .base_to_hit = rtohit,
                                                       .attack_type = PA_SAW,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = RARM});
    }
  }
  if (!using) {
    mech_notify(mech, MECHALL, "You don't have a dual saw!");
    return;
  }
} // end mech_saw()
void mech_claw(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int ltohit = 4;
  int rtohit = 4;
  int using = P_LEFT | P_RIGHT;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "claw"))
    return;
  if (!physical_quad_check(player, mech, "claw"))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    rtohit = ltohit = find_pilot_piloting(mech);
  ArmSelectionResult selection = physical_arm_select(&(ArmSelectionRequest){
      .using = using,
      .argument_count = argc,
      .arguments = args,
      .mech = mech,
      .has_weapon = have_claw,
      .weapon = "a claw",
  });
  if (selection.failed) {
    return;
  }
  using = selection.using;
  argc = selection.argument_count;
  args = selection.arguments;
  if (!phys_common_checks(mech))
    return;
  if (using & P_LEFT) {
    physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                     .damage_weight = 7,
                                                     .base_to_hit = ltohit,
                                                     .attack_type = PA_CLAW,
                                                     .argument_count = argc,
                                                     .arguments = args,
                                                     .map = mech_map,
                                                     .section = LARM});
  }
  if (using & P_RIGHT) {
    physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                     .damage_weight = 7,
                                                     .base_to_hit = rtohit,
                                                     .attack_type = PA_CLAW,
                                                     .argument_count = argc,
                                                     .arguments = args,
                                                     .map = mech_map,
                                                     .section = RARM});
  }
  if (!using) {
    mech_notify(mech, MECHALL,
                "You do not have any claws! Try punching/clubbing instead!");
    return;
  }
} // end mech_claw()
static bool mace_check_arm(Mech *mech, int arm) {
  const char *arm_used = (arm == RARM ? "right" : "left");
  if (mech_section_is_destroyed(mech, arm)) {
    mech_printf(mech, MECHALL,
                "Your %s arm is destroyed, you can't use a mace with it.",
                arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 0},
                                  .special = SHOULDER_OR_HIP})) {
    mech_printf(
        mech, MECHALL,
        "Your %s shoulder is destroyed, you can't use a mace with that arm.",
        arm_used);
    return false;
  }
  if (!mech_critical_is_operational_special(
          &(CriticalSpecialCheck){.mech = mech,
                                  .slot = {.section = arm, .critical = 3},
                                  .special = HAND_OR_FOOT_ACTUATOR})) {
    mech_printf(
        mech, MECHALL,
        "Your %s hand is destroyed, you can't use a mace with that arm.",
        arm_used);
    return false;
  }
  return true;
} // end mace_checkArm()
void mech_mace(DbRef player, Mech *mech, char *buffer) {
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *argl[5];
  char **args = argl;
  int argc;
  int ltohit = 4;
  int rtohit = 4;
  int using = P_LEFT | P_RIGHT;
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!physical_arm_check(player, mech, "mace"))
    return;
  if (!physical_quad_check(player, mech, "mace"))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (btech_context_physical_attacks_use_pilot_skill(mech_context(mech)))
    ltohit = rtohit = find_pilot_piloting(mech) - 1;
  ArmSelectionResult selection = physical_arm_select(&(ArmSelectionRequest){
      .using = using,
      .argument_count = argc,
      .arguments = args,
      .mech = mech,
      .has_weapon = have_mace,
      .weapon = "a mace",
  });
  if (selection.failed) {
    return;
  }
  using = selection.using;
  argc = selection.argument_count;
  args = selection.arguments;
  if (using & P_LEFT) {
    if (mace_check_arm(mech, LARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 4,
                                                       .base_to_hit = ltohit,
                                                       .attack_type = PA_MACE,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = LARM});
    }
  }
  if (using & P_RIGHT) {
    if (mace_check_arm(mech, RARM)) {
      physical_attack_resolve(&(PhysicalAttackRequest){.mech = mech,
                                                       .damage_weight = 4,
                                                       .base_to_hit = rtohit,
                                                       .attack_type = PA_MACE,
                                                       .argument_count = argc,
                                                       .arguments = args,
                                                       .map = mech_map,
                                                       .section = RARM});
    }
  }
  if (!using) {
    mech_notify(mech, MECHALL, "You don't have a mace!");
    return;
  }
} // end mech_mace()
