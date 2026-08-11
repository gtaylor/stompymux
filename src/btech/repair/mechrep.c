/* Implements BattleTech repair mechanics for mechrep. */

#include <stdarg.h>
#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_terrain.h"    // IWYU pragma: keep
#include "mech_lifecycle.h" // IWYU pragma: keep
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"

#include "checked_conversion.h"

#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "special_object.h"
#include "template_api.h"

/* Selectors */
extern char *strtok(char *s, const char *ct);

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

void newfreemechrep(DbRef key, void **data,
                    BtechSpecialLifecycleOperation selector) {
  struct RepairFacility *new = *data;

  switch (selector) {
  case SPECIAL_ALLOC:
    new->mynum = key;
    new->current_target = -1;
    break;
  case SPECIAL_FREE:
    break;
  }
}

/* With cap R means restricted command */

void mechrep_rresetcrits(DbRef player, void *data, char *buffer) {
  int i;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Default criticals set!");
  for (i = 0; i < NUM_SECTIONS; i++)
    fill_default_criticals(mech, i);
}

void mechrep_rdisplaysection(DbRef player, void *data, char *buffer) {
  char *args[1];
  int index;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "You must specify a section to list the criticals for!");
    return;
  }
  index = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    args[0]);
  if (index == -1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid section!");
    return;
  }
  critical_status(btech_context_evaluation(rep->xcode.context), player, mech,
                  index);
}

void mechrep_rsetradio(DbRef player, void *data, char *buffer) {
  char *args[2];
  int i;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  switch (mech_parseattributes(buffer, args, 2)) {
  case 0:
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "This remains to be done [showing of stuff when no args]");
    return;
  case 2:
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Too many args, unable to cope().");
    return;
  }
  if (!parse_int_checked(args[0], &i)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid radio level!");
    return;
  }
  i = bounded(1, i, 5);
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Radio level set to %d.", i);
  mech_radio_quality_set(mech, i);
  mech_radio_configuration_set(mech, generic_radio_type(i, 0));
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Number of freqs: %d  Extra stuff: %d",
                mech_radio_configuration(mech) % 16,
                (mech_radio_configuration(mech) / 16) * 16);
  mech_radio_range_set(
      mech,
      clamp_float_to_int(DEFAULT_RADIORANGE * generic_radio_multiplier(mech)));
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Radio range set to %d.", mech_radio_range(mech));
}

void mechrep_rsettarget(DbRef player, void *data, char *buffer) {
  char *args[2];
  DbRef newmech;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, false,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  switch (mech_parseattributes(buffer, args, 2)) {
  case 1:
    newmech = match_thing(&btech_context_command(rep->xcode.context)->match,
                          player, args[0]);
    if (!(is_good_obj(rep->xcode.context->database, newmech) &&
          is_xcode(rep->xcode.context->database, newmech))) {
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "That is not a BattleMech or Vehicle!");
      return;
    }
    rep->current_target = newmech;
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Mech to repair changed to #%ld", newmech);
    break;
  default:
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Too many arguments!");
  }
}

void mechrep_rsettype(DbRef player, void *data, char *buffer) {
  char *args[1];

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  char *movement =
      *(char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
  switch (ascii_to_upper(*movement)) {
  case 'M':
    mech_class_set(mech, CLASS_MECH);
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to MECH");
    break;
  case 'Q':
    mech_class_set(mech, CLASS_MECH);
    mech_movement_type_set(mech, MOVE_QUAD);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to QUAD");
    break;
  case 'G':
    mech_class_set(mech, CLASS_VEH_GROUND);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to VEHICLE");
    break;
  case 'V':
    mech_class_set(mech, CLASS_VTOL);
    mech_movement_type_set(mech, MOVE_VTOL);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to VTOL");
    break;
  case 'N':
    mech_class_set(mech, CLASS_VEH_NAVAL);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to NAVAL");
    break;
  case 'A':
    mech_class_set(mech, CLASS_AERO);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to AeroSpace");
    break;
  case 'D':
    mech_class_set(mech, CLASS_DS);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to DropShip");
    break;
  case 'S':
    mech_class_set(mech, CLASS_SPHEROID_DS);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to SpheroidDropship");
    break;
  case 'B':
    mech_class_set(mech, CLASS_BSUIT);
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Type set to BattleSuit");
    break;
  default:
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Types are: MECH, GROUND, VTOL, NAVAL, AERO, DROPSHIP and "
                 "SPHEROIDDROPSHIP");
    break;
  }
}

static bool parse_repair_float(RepairFacility *rep, DbRef player, char *buffer,
                               const char *name, float *value) {
  char *args[1];
  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Invalid number of arguments to Set%s!", name);
    return false;
  }
  *value = strtof(args[0], nullptr);
  return true;
}

static bool parse_repair_int(RepairFacility *rep, DbRef player, char *buffer,
                             const char *name, int *value) {
  char *args[1];
  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Invalid number of arguments to Set%s!", name);
    return false;
  }
  return parse_int_checked(args[0], value);
}

static void notify_repair_float(RepairFacility *rep, DbRef player,
                                const char *name, float value) {
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "%s changed to %.2f.", name, (double)value);
}

static void notify_repair_int(RepairFacility *rep, DbRef player,
                              const char *name, int value) {
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "%s changed to %d.", name, value);
}

void mechrep_rsetspeed(DbRef player, void *data, char *buffer) {
  float value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_float(rep, player, buffer, "Maxspeed", &value))
    return;
  value *= KPH_PER_MP;
  mech_maximum_speed_set(mech, value);
  notify_repair_float(rep, player, "Maxspeed", mech_maximum_speed(mech));
}

void mechrep_rsetjumpspeed(DbRef player, void *data, char *buffer) {
  float value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_float(rep, player, buffer, "Jumpspeed", &value))
    return;
  value *= KPH_PER_MP;
  mech_jump_speed_set(mech, value);
  notify_repair_float(rep, player, "Jumpspeed", mech_jump_speed(mech));
}

void mechrep_rsetheatsinks(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "Heatsinks", &value))
    return;
  mech_heat_sink_count_set(mech, value);
  notify_repair_int(rep, player, "Heatsinks", mech_heat_sink_count(mech));
}

void mechrep_rsetlrsrange(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "LRSrange", &value))
    return;
  mech_long_range_sensor_range_set(mech, value);
  notify_repair_int(rep, player, "LRSrange",
                    mech_long_range_sensor_range(mech));
}

void mechrep_rsettacrange(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "TACrange", &value))
    return;
  mech_tactical_range_set(mech, value);
  notify_repair_int(rep, player, "TACrange", mech_tactical_range(mech));
}

void mechrep_rsetscanrange(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "SCANrange", &value))
    return;
  mech_scanner_range_set(mech, value);
  notify_repair_int(rep, player, "SCANrange", mech_scanner_range(mech));
}

void mechrep_rsetradiorange(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "RADIOrange", &value))
    return;
  mech_radio_range_set(mech, value);
  notify_repair_int(rep, player, "RADIOrange", mech_radio_range(mech));
}

void mechrep_rsettons(DbRef player, void *data, char *buffer) {
  int value;
  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(rep, player, buffer, "Tons", &value))
    return;
  mech_tonnage_set(mech, value);
  notify_repair_int(rep, player, "Tons", mech_tonnage(mech));
}

void mechrep_rsetmove(DbRef player, void *data, char *buffer) {
  char *args[1];

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  char *movement =
      *(char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
  switch (ascii_to_upper(*movement)) {
  case 'T':
    mech_movement_type_set(mech, MOVE_TRACK);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to TRACKED");
    break;
  case 'W':
    mech_movement_type_set(mech, MOVE_WHEEL);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to WHEELED");
    break;
  case 'H':
    switch (ascii_to_upper(*checked_string_suffix(movement, 1))) {
    case 'O':
      mech_movement_type_set(mech, MOVE_HOVER);
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Movement set to HOVER");
      break;
    case 'U':
      mech_movement_type_set(mech, MOVE_HULL);
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Movement set to HULL");
      break;
    }
    break;
  case 'V':
    mech_movement_type_set(mech, MOVE_VTOL);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to VTOL");
    break;
  case 'Q':
    mech_movement_type_set(mech, MOVE_QUAD);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to QUAD");
    break;
  case 'B':
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to BIPED");
    break;
  case 'S':
    mech_movement_type_set(mech, MOVE_SUB);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to SUB");
    break;
  case 'F':
    switch (ascii_to_upper(*checked_string_suffix(movement, 1))) {
    case 'O':
      mech_movement_type_set(mech, MOVE_FOIL);
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Movement set to FOIL");
      break;
    case 'L':
      mech_movement_type_set(mech, MOVE_FLY);
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Movement set to FLY");
      break;
    }
    break;
  case 'N':
    mech_movement_type_set(mech, MOVE_NONE);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Movement set to NONE");
    break;
  default:
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Types are: TRACK, WHEEL, VTOL, QUAD, BIPED, HOVER, HULL, "
                 "FLY, SUB, FOIL and NONE");
    break;
  }
}
