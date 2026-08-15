#include "mech_notify_api.h"

#include <stdio.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void mech_show_flag(EvaluationContext *evaluation, DbRef player,
                           int spaces, const char *text) {
  notify_printf(evaluation, player, "%*s%s", spaces, "", text);
}

const char *get_ammo_desc_model_mode(int model, int mode) {
  if (mode & LBX_MODE)
    return " Shotgun";
  if (mode & ARTEMIS_MODE)
    return " Artemis IV";
  if (mode & NARC_MODE)
    return weapon_catalogue_has_special(model, NARC) ? " Explosive" : " Narc";
  if (mode & INARC_EXPLO_MODE)
    return " iExplosive";
  if (mode & INARC_HAYWIRE_MODE)
    return " Haywire";
  if (mode & INARC_ECM_MODE)
    return " ECM";
  if (mode & INARC_NEMESIS_MODE)
    return " Nemesis";
  if (mode & SWARM_MODE)
    return " Swarm";
  if (mode & SWARM1_MODE)
    return " Swarm1";
  if (mode & INFERNO_MODE)
    return " Inferno";
  if (mode & CLUSTER_MODE)
    return " Cluster";
  if (mode & SMOKE_MODE)
    return " Smoke";
  if (mode & MINE_MODE)
    return " Mine";
  if (mode & AC_AP_MODE)
    return " Armor Piercing";
  if (mode & AC_FLECHETTE_MODE)
    return " Flechette";
  if (mode & AC_INCENDIARY_MODE)
    return " Incendiary";
  if (mode & AC_PRECISION_MODE)
    return " Precision";
  if (mode & STINGER_MODE)
    return " Stinger";
  if (mode & AC_CASELESS_MODE)
    return " Caseless";
  if (mode & SGUIDED_MODE)
    return " Sguided";
  if (mode & ATM_ER_MODE)
    return " ExtendedRange";
  if (mode & ATM_HE_MODE)
    return " HighExplosive";
  if (mode & MML_LRM_MODE)
    return " LRM";
  return "";
}

char get_weapon_ammo_mode_letter_model_mode(int model, unsigned int mode) {
  if (!(mode & AMMO_MODES))
    return ' ';
  if (mode & CLUSTER_MODE)
    return 'C';
  if (mode & SMOKE_MODE)
    return 'S';
  if (mode & MINE_MODE)
    return 'M';
  if (mode & LBX_MODE)
    return 'L';
  if (mode & ARTEMIS_MODE)
    return 'A';
  if (mode & NARC_MODE)
    return weapon_catalogue_has_special(model, NARC) ? 'E' : 'N';
  if (mode & INARC_EXPLO_MODE)
    return 'X';
  if (mode & INARC_HAYWIRE_MODE)
    return 'Y';
  if (mode & INARC_ECM_MODE)
    return 'E';
  if (mode & INARC_NEMESIS_MODE)
    return 'Z';
  if (mode & INFERNO_MODE)
    return 'I';
  if (mode & SWARM_MODE)
    return 'W';
  if (mode & SWARM1_MODE)
    return '1';
  if (mode & AC_AP_MODE)
    return 'R';
  if (mode & AC_FLECHETTE_MODE)
    return 'F';
  if (mode & AC_INCENDIARY_MODE)
    return 'D';
  if (mode & AC_PRECISION_MODE)
    return 'P';
  if (mode & STINGER_MODE)
    return 'T';
  if (mode & AC_CASELESS_MODE)
    return 'U';
  if (mode & SGUIDED_MODE)
    return 'G';
  if (mode & ATM_ER_MODE)
    return 'R';
  if (mode & ATM_HE_MODE)
    return 'X';
  if (mode & MML_LRM_MODE)
    return '#';
  return ' ';
}

char get_weapon_fire_mode_letter_model_mode(int model [[maybe_unused]],
                                            int mode) {
  if (!(mode & FIRE_MODES))
    return ' ';
  if (mode & HOTLOAD_MODE)
    return 'H';
  if (mode & ULTRA_MODE)
    return 'U';
  if (mode & RFAC_MODE)
    return 'F';
  if (mode & GATTLING_MODE)
    return 'G';
  if (mode & HEAT_MODE)
    return 'H';
  if (mode & RAC_TWOSHOT_MODE)
    return '2';
  if (mode & RAC_FOURSHOT_MODE)
    return '4';
  if (mode & RAC_SIXSHOT_MODE)
    return '6';
  return ' ';
}

char get_weapon_ammo_mode_letter(Mech *mech, int loop, int crit) {
  const int MODE = mech_critical_ammo_mode(mech, loop, crit);

  if (MODE < 0)
    return ' ';
  return get_weapon_ammo_mode_letter_model_mode(
      weapon_from_equipment_index(mech_critical_part_type(mech, loop, crit)),
      (unsigned int)MODE);
}

char get_weapon_fire_mode_letter(Mech *mech, int loop, int crit) {
  return get_weapon_fire_mode_letter_model_mode(
      weapon_from_equipment_index(mech_critical_part_type(mech, loop, crit)),
      mech_critical_fire_mode(mech, loop, crit));
}

const char *get_move_type_id(int movetype) {
  switch (movetype) {
  case MOVE_QUAD:
    return "QUAD";
  case MOVE_BIPED:
    return "BIPED";
  case MOVE_TRACK:
    return "TRACKED";
  case MOVE_WHEEL:
    return "WHEELED";
  case MOVE_HOVER:
    return "HOVER";
  case MOVE_VTOL:
    return "VTOL";
  case MOVE_FLY:
    return "FLY";
  case MOVE_HULL:
    return "HULL";
  case MOVE_SUB:
    return "SUBMARINE";
  case MOVE_FOIL:
    return "HYDROFOIL";
  default:
    return "Unknown";
  }
}

void mech_show_flags(const MechFlagDisplayRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  const DbRef PLAYER = request->player;
  Mech *mech = request->mech;
  const int SPACES = request->indentation;
  const int LEVEL = request->detail_level;
  MechConditionSummary conditions = mech_condition_summary(mech);

  if (conditions.combat_safe) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=blue bold]COMBAT SAFE[reset]");
  }
  if (conditions.fortified) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=green bold]FORTIFIED[reset]");
  }
  if (conditions.weapons_hold) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=red bold]WEAPONS HOLD[reset]");
  }
  if (mech_is_fallen(mech)) {
    const char *fallen_text = "";
    switch (mech_movement_type(mech)) {
    case MOVE_BIPED:
    case MOVE_QUAD:
      fallen_text = "[fg=red bold]FALLEN[reset]";
      break;
    case MOVE_TRACK:
      fallen_text = "[fg=red bold]TRACK DESTROYED[reset]";
      break;
    case MOVE_WHEEL:
      fallen_text = "[fg=red bold]AXLE DESTROYED[reset]";
      break;
    case MOVE_HOVER:
      fallen_text = "[fg=red bold]LIFT FAN DESTROYED[reset]";
      break;
    case MOVE_VTOL:
      fallen_text = "[fg=red bold]ROTOR DESTROYED[reset]";
      break;
    case MOVE_FLY:
      fallen_text = "[fg=red bold]ENGINE DESTROYED[reset]";
      break;
    case MOVE_HULL:
    case MOVE_SUB:
      fallen_text = "[fg=red bold]ENGINE ROOM DESTROYED[reset]";
      break;
    case MOVE_FOIL:
      fallen_text = "[fg=red bold]FOIL DESTROYED[reset]";
      break;
    case MOVE_NONE:
      break;
    }
    mech_show_flag(evaluation, PLAYER, SPACES, fallen_text);
  }
  if (conditions.hull_down) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=green bold]HULLDOWN[reset]");
  }
  if (conditions.dug_in) {
    mech_show_flag(evaluation, PLAYER, SPACES, "[fg=green bold]DUG IN[reset]");
  }
  if (conditions.digging) {
    mech_show_flag(evaluation, PLAYER, SPACES, "[fg=green]DIGGING IN[reset]");
  }
  if (conditions.staggering) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=red bold]STAGGERING[reset]");
  }
  if (conditions.searchlight_destroyed) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=red bold]SEARCHLIGHT DESTROYED[reset]");
  }
  if (mech_searchlight_active(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=green bold]SEARCHLIGHT ON[reset]");
  } else if (conditions.illuminated) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=green bold]ILLUMINATED[reset]");
  }
  if (mech_event_count(mech, EVENT_VEHICLEBURN) || mech_is_jellied(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES, "[fg=red bold]ON FIRE[reset]");
  }
  if (conditions.hidden) {
    mech_show_flag(evaluation, PLAYER, SPACES, "[fg=green bold]HIDDEN[reset]");
  }
  if (bsuit_has_enemy_swarmers(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=red bold]SWARMED BY ENEMY SUITS[reset]");
  }
  if (bsuit_has_friendly_riders(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES,
                   "[fg=red bold]MOUNTED BY FRIENDLY SUITS[reset]");
  }
  if (conditions.swarm_target > 0) {
    Mech *swarm_target =
        btech_context_get_mech(mech_context(mech), conditions.swarm_target);
    if (swarm_target) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     mech_team(swarm_target) == mech_team(mech)
                         ? "[fg=green bold]MOUNTED ON FRIENDLY UNIT[reset]"
                         : "[fg=green bold]SWARMING ENEMY UNIT[reset]");
    }
  }
  if (LEVEL == 0) { /* our own 'status' */
    if (conditions.ecm_protected) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]PROTECTED BY ECM[reset]");
    }
    if (conditions.angel_ecm_protected) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]PROTECTED BY ANGEL ECM[reset]");
    }
    if (mech_is_ecm_disturbed(mech)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]AFFECTED BY ECM[reset]");
    }
    if (conditions.angel_ecm_disturbed) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]AFFECTED BY ANGEL ECM[reset]");
    }
    if (conditions.ecm_countered) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]COUNTERED BY ECCM[reset]");
    }
    if (conditions.stealth_armor_active) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]STEALTH ARMOR ACTIVE[reset]");
    }
    if (conditions.null_signature_active) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]NULL SIGNATURE SYSTEM ACTIVE[reset]");
    }
    if (check_all_sections(mech, NARC_ATTACHED)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]NARC POD ATTACHED[reset]");
    }
    if (check_all_sections(mech, INARC_HOMING_ATTACHED)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]INARC HOMING POD ATTACHED[reset]");
    }
    if (check_all_sections(mech, INARC_HAYWIRE_ATTACHED)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]INARC HAYWIRE POD ATTACHED[reset]");
    }
    if (check_all_sections(mech, INARC_ECM_ATTACHED)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]INARC ECM POD ATTACHED[reset]");
    }
    if (mech_event_count(mech, EVENT_VEHICLE_EXTINGUISH)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=yellow bold]EXTINGUISHING FIRE[reset]");
    }
    if (conditions.turret_auto_turn) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]TURRET AUTO-TURN ENGAGED[reset]");
    }
    if (mech_section_carries_club(mech, RARM)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]CARRYING CLUB - RIGHT ARM[reset]");
    }
    if (mech_section_carries_club(mech, LARM)) {
      mech_show_flag(evaluation, PLAYER, SPACES,
                     "[fg=green bold]CARRYING CLUB - LEFT ARM[reset]");
    }
  }
  if (LEVEL <= 1 && mech_is_destroyed(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES, "DESTROYED");
  }
  if (LEVEL <= 1 && !mech_is_started(mech)) {
    mech_show_flag(evaluation, PLAYER, SPACES, "SHUTDOWN");
  }
  if (LEVEL == 0 && conditions.torso_right) {
    mech_show_flag(evaluation, PLAYER, SPACES, "Torso is 60 degrees right");
  }
  if (LEVEL == 0 && conditions.torso_left) {
    mech_show_flag(evaluation, PLAYER, SPACES, "Torso is 60 degrees left");
  }
}

const char *get_arc_id(Mech *mech, int arc) {
  int unit_class = mech_class(mech);
  bool mechlike = (unit_class == CLASS_MECH || unit_class == CLASS_MW ||
                   unit_class == CLASS_BSUIT) != 0;

  if (arc & FORWARDARC)
    return "Forward";
  if (arc & RSIDEARC)
    return mechlike ? "Right Arm" : "Right Side";
  if (arc & LSIDEARC)
    return mechlike ? "Left Arm" : "Left Side";
  if (arc & REARARC)
    return "Rear";
  return "NO";
}

MechDisplayId mech_to_mech_display_id_base(Mech *see, Mech *mech, int inlos) {
  const char *mname;
  MechDisplayId id = {0};

  BtechContext *context = mech_context(mech);
  DbRef object = mech_dbref(mech);
  if (!is_good_obj(context->database, object))
    return id;

  if (!inlos)
    mname = "something";
  else
    mname = btech_attribute_read(context->database, object, A_MECHNAME,
                                 (char[LBUF_SIZE]){0});

  (void)snprintf(
      id.text, sizeof(id.text), "%s [%s]", mname,
      mech_id(mech, (inlos && mech_team(see) == mech_team(mech)) != 0).text);
  return id;
}

MechDisplayId mech_to_mech_display_id(Mech *see, Mech *mech) {
  const char *mname;
  int team;
  MechDisplayId id = {0};

  if (!mech) {
    return id;
  }
  if (!see) {
    return id;
  }
  BtechContext *context = mech_context(mech);
  DbRef object = mech_dbref(mech);
  if (!is_good_obj(context->database, object))
    return id;

  if (!mech_los_check_unblocked(see, mech, 0, 0, 0)) {
    mname = "something";
    team = 0;
  } else {
    mname = btech_attribute_read(context->database, object, A_MECHNAME,
                                 (char[LBUF_SIZE]){0});
    team = mech_team(see) == mech_team(mech);
  }

  (void)snprintf(id.text, sizeof(id.text), "%s [%s]", mname,
                 mech_id(mech, team != 0).text);
  return id;
}

MechDisplayId mech_display_id(Mech *mech) {
  char *mname;
  MechDisplayId id = {0};

  BtechContext *context = mech_context(mech);
  DbRef object = mech_dbref(mech);
  if (!is_good_obj(context->database, object))
    return id;

  mname = btech_attribute_read(context->database, object, A_MECHNAME,
                               (char[LBUF_SIZE]){0});
  (void)snprintf(id.text, sizeof(id.text), "%s [%s]", mname,
                 mech_id(mech, false).text);
  return id;
}
