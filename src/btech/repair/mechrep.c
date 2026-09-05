/* Implements BattleTech repair mechanics for mechrep. */

#include <limits.h>
#include <math.h>
#include <stdarg.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "map_terrain.h"    // IWYU pragma: keep
#include "mech_lifecycle.h" // IWYU pragma: keep
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
#include "mechrep_api.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "special_object.h"
#include "template_api.h"

/* Selectors */

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* With cap R means restricted command */

void mechrep_rresetcrits(DbRef player, void *data,
                         char *buffer [[maybe_unused]]) {
  int i;

  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  mecha_notify(btech_context_evaluation(context), player,
               "Default criticals set!");
  for (i = 0; i < NUM_SECTIONS; i++)
    fill_default_criticals(mech, i);
}

void mechrep_rdisplaysection(DbRef player, void *data, char *buffer) {
  char *args[1];
  int index;

  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must specify a section to list the criticals for!");
    return;
  }
  index = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    args[0]);
  if (index == -1) {
    mecha_notify(btech_context_evaluation(context), player, "Invalid section!");
    return;
  }
  critical_status(btech_context_evaluation(context), player, mech, index);
}

void mechrep_rsetradio(DbRef player, void *data, char *buffer) {
  char *args[2];
  int i;

  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  switch (mech_parseattributes(buffer, args, 2)) {
  case 0:
    mecha_notify(btech_context_evaluation(context), player,
                 "This remains to be done [showing of stuff when no args]");
    return;
  case 2:
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many args, unable to cope().");
    return;
  }
  if (!parse_int_checked(args[0], &i)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid radio level!");
    return;
  }
  i = bounded(1, i, 5);
  notify_printf(btech_context_evaluation(context), player,
                "Radio level set to %d.", i);
  mech_radio_quality_set(mech, i);
  mech_radio_configuration_set(mech, generic_radio_type(i, 0));
  notify_printf(btech_context_evaluation(context), player,
                "Number of freqs: %d  Extra stuff: %d",
                mech_radio_configuration(mech) % 16,
                (mech_radio_configuration(mech) / 16) * 16);
  mech_radio_range_set(
      mech,
      clamp_float_to_int(DEFAULT_RADIORANGE * generic_radio_multiplier(mech)));
  notify_printf(btech_context_evaluation(context), player,
                "Radio range set to %d.", mech_radio_range(mech));
}

void mechrep_rsettype(DbRef player, void *data, char *buffer) {
  char *args[1];

  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return;
  }
  char *movement =
      *(char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
  switch (ascii_to_upper(*movement)) {
  case 'M':
    mech_class_set(mech, CLASS_MECH);
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(context), player, "Type set to MECH");
    break;
  case 'Q':
    mech_class_set(mech, CLASS_MECH);
    mech_movement_type_set(mech, MOVE_QUAD);
    mecha_notify(btech_context_evaluation(context), player, "Type set to QUAD");
    break;
  case 'G':
    mech_class_set(mech, CLASS_VEH_GROUND);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to VEHICLE");
    break;
  case 'V':
    mech_class_set(mech, CLASS_VTOL);
    mech_movement_type_set(mech, MOVE_VTOL);
    mecha_notify(btech_context_evaluation(context), player, "Type set to VTOL");
    break;
  case 'N':
    mech_class_set(mech, CLASS_VEH_NAVAL);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to NAVAL");
    break;
  case 'A':
    mech_class_set(mech, CLASS_AERO);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to AeroSpace");
    break;
  case 'D':
    mech_class_set(mech, CLASS_DS);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to DropShip");
    break;
  case 'S':
    mech_class_set(mech, CLASS_SPHEROID_DS);
    mech_movement_type_set(mech, MOVE_FLY);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to SpheroidDropship");
    break;
  case 'B':
    mech_class_set(mech, CLASS_BSUIT);
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(context), player,
                 "Type set to BattleSuit");
    break;
  default:
    mecha_notify(btech_context_evaluation(context), player,
                 "Types are: MECH, GROUND, VTOL, NAVAL, AERO, DROPSHIP and "
                 "SPHEROIDDROPSHIP");
    break;
  }
}

static bool parse_repair_float(BtechContext *context, DbRef player,
                               char *buffer, const char *name, float *value) {
  char *args[2];
  if (mech_parseattributes(buffer, args, 2) != 1 ||
      !parse_float_checked(args[0], value)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Invalid value for Set%s!", name);
    return false;
  }
  return true;
}

static bool parse_repair_int(BtechContext *context, DbRef player, char *buffer,
                             const char *name, int *value) {
  char *args[2];
  if (mech_parseattributes(buffer, args, 2) != 1 ||
      !parse_int_checked(args[0], value)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Invalid value for Set%s!", name);
    return false;
  }
  return true;
}

static bool validate_repair_int_range(BtechContext *context, DbRef player,
                                      const char *name, int value, int minimum,
                                      int maximum) {
  if (value >= minimum && value <= maximum)
    return true;

  notify_printf(btech_context_evaluation(context), player,
                "Invalid value for Set%s!", name);
  return false;
}

static void notify_repair_float(BtechContext *context, DbRef player,
                                const char *name, float value) {
  notify_printf(btech_context_evaluation(context), player,
                "%s changed to %.2f.", name, (double)value);
}

static void notify_repair_int(BtechContext *context, DbRef player,
                              const char *name, int value) {
  notify_printf(btech_context_evaluation(context), player, "%s changed to %d.",
                name, value);
}

void mechrep_rsetspeed(DbRef player, void *data, char *buffer) {
  float value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_float(context, player, buffer, "Maxspeed", &value))
    return;
  value *= KPH_PER_MP;
  if (value < 0.0F || !isfinite(value)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Invalid value for SetMaxspeed!");
    return;
  }
  mech_maximum_speed_set(mech, value);
  notify_repair_float(context, player, "Maxspeed", mech_maximum_speed(mech));
}

void mechrep_rsetjumpspeed(DbRef player, void *data, char *buffer) {
  float value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_float(context, player, buffer, "Jumpspeed", &value))
    return;
  value *= KPH_PER_MP;
  if (value < 0.0F || !isfinite(value)) {
    notify_printf(btech_context_evaluation(context), player,
                  "Invalid value for SetJumpspeed!");
    return;
  }
  mech_jump_speed_set(mech, value);
  notify_repair_float(context, player, "Jumpspeed", mech_jump_speed(mech));
}

void mechrep_rsetheatsinks(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "Heatsinks", &value))
    return;
  if (!validate_repair_int_range(context, player, "Heatsinks", value, 0,
                                 CHAR_MAX))
    return;
  mech_heat_sink_count_set(mech, value);
  notify_repair_int(context, player, "Heatsinks", mech_heat_sink_count(mech));
}

void mechrep_rsetlrsrange(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "LRSrange", &value))
    return;
  if (!validate_repair_int_range(context, player, "LRSrange", value, 0,
                                 CHAR_MAX))
    return;
  mech_long_range_sensor_range_set(mech, value);
  notify_repair_int(context, player, "LRSrange",
                    mech_long_range_sensor_range(mech));
}

void mechrep_rsettacrange(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "TACrange", &value))
    return;
  if (!validate_repair_int_range(context, player, "TACrange", value, 0,
                                 CHAR_MAX))
    return;
  mech_tactical_range_set(mech, value);
  notify_repair_int(context, player, "TACrange", mech_tactical_range(mech));
}

void mechrep_rsetscanrange(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "SCANrange", &value))
    return;
  if (!validate_repair_int_range(context, player, "SCANrange", value, 0,
                                 CHAR_MAX))
    return;
  mech_scanner_range_set(mech, value);
  notify_repair_int(context, player, "SCANrange", mech_scanner_range(mech));
}

void mechrep_rsetradiorange(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "RADIOrange", &value))
    return;
  if (!validate_repair_int_range(context, player, "RADIOrange", value, 0,
                                 SHRT_MAX))
    return;
  mech_radio_range_set(mech, value);
  notify_repair_int(context, player, "RADIOrange", mech_radio_range(mech));
}

void mechrep_rsettons(DbRef player, void *data, char *buffer) {
  int value;
  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (!parse_repair_int(context, player, buffer, "Tons", &value))
    return;
  if (!validate_repair_int_range(context, player, "Tons", value, 1, INT_MAX))
    return;
  mech_tonnage_set(mech, value);
  notify_repair_int(context, player, "Tons", mech_tonnage(mech));
}

void mechrep_rsetmove(DbRef player, void *data, char *buffer) {
  char *args[1];

  MechAdminCommandContext repair_command;
  RepairCommandStatus repair_status =
      mech_admin_command_context_initialize(player, data, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  BtechContext *context = repair_command.context;
  Mech *mech = repair_command.mech;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return;
  }
  char *movement =
      *(char **)checked_storage_at((void *)args, 1, sizeof(*args), 0);
  switch (ascii_to_upper(*movement)) {
  case 'T':
    mech_movement_type_set(mech, MOVE_TRACK);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to TRACKED");
    break;
  case 'W':
    mech_movement_type_set(mech, MOVE_WHEEL);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to WHEELED");
    break;
  case 'H':
    switch (ascii_to_upper(*checked_string_suffix(movement, 1))) {
    case 'O':
      mech_movement_type_set(mech, MOVE_HOVER);
      mecha_notify(btech_context_evaluation(context), player,
                   "Movement set to HOVER");
      break;
    case 'U':
      mech_movement_type_set(mech, MOVE_HULL);
      mecha_notify(btech_context_evaluation(context), player,
                   "Movement set to HULL");
      break;
    }
    break;
  case 'V':
    mech_movement_type_set(mech, MOVE_VTOL);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to VTOL");
    break;
  case 'Q':
    mech_movement_type_set(mech, MOVE_QUAD);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to QUAD");
    break;
  case 'B':
    mech_movement_type_set(mech, MOVE_BIPED);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to BIPED");
    break;
  case 'S':
    mech_movement_type_set(mech, MOVE_SUB);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to SUB");
    break;
  case 'F':
    switch (ascii_to_upper(*checked_string_suffix(movement, 1))) {
    case 'O':
      mech_movement_type_set(mech, MOVE_FOIL);
      mecha_notify(btech_context_evaluation(context), player,
                   "Movement set to FOIL");
      break;
    case 'L':
      mech_movement_type_set(mech, MOVE_FLY);
      mecha_notify(btech_context_evaluation(context), player,
                   "Movement set to FLY");
      break;
    }
    break;
  case 'N':
    mech_movement_type_set(mech, MOVE_NONE);
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement set to NONE");
    break;
  default:
    mecha_notify(btech_context_evaluation(context), player,
                 "Types are: TRACK, WHEEL, VTOL, QUAD, BIPED, HOVER, HULL, "
                 "FLY, SUB, FOIL and NONE");
    break;
  }
}
