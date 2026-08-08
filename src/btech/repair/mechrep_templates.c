/*
 * Last modified: Thu Aug 13 23:41:12 1998 fingon
 * Copyright (c) 1999-2005 Kevin Stevens
 *   All right reserved
 */

#include <ctype.h>
#include <dirent.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_events.h"
#include "mech_lifecycle.h" // IWYU pragma: keep
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "repair_job.h"

#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_restrict_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_template_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
#include "template_api.h"

/* Selectors */
extern char *strtok(char *s, const char *ct);

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

void mechrep_Rloadnew(DbRef player, void *data, char *buffer) {
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
  if (mech_parseattributes(buffer, args, 1) == 1)
    if (mech_template_load(player, mech, args[0]) == 1) {
      mech_events_cancel_all(mech);
      clear_mech_from_LOS(mech);
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Template loaded.");
      return;
    }
  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Unable to read that template.");
}

void mechrep_Rrestore(DbRef player, void *data, char *buffer) {
  char *c;

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
  c = btech_attribute_read(btech_context_database(mech_context(mech)),
                           mech_dbref(mech), A_MECHREF, (char[LBUF_SIZE]){0});
  if (!c || !*c) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Sorry, I don't know what type of mech this is");
    return;
  }
  if (mech_template_load(player, mech, c) == 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Restoration complete!");
    return;
  }
  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Unable to restore this mech!.");
}

void mechrep_Rsavetemp(DbRef player, void *data, char *buffer) {
  char *args[1];
  FILE *fp;
  char openfile[512] = {0};
  int i, j;

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

  mech_template_registry_clear(mech_context(mech));

  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "You must specify a template name!");
    return;
  }
  if (strstr(args[0], "/")) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid file name!");
    return;
  }
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Saving %s...", args[0]);
  snprintf(openfile, sizeof(openfile), "%s/",
           btech_context_mech_template_path(mech_context(mech)));
  strlcat(openfile, args[0], sizeof(openfile));
  if (!(fp = fopen(openfile, "w"))) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Unable to open/create mech file! Sorry.");
    return;
  }
  float const maximum_speed = mech_maximum_speed(mech);
  float const jump_speed = mech_jump_speed(mech);
  fprintf(fp, "%d %d %d %d %d %.2f %.2f %d\n", mech_tonnage(mech),
          mech_tactical_range(mech), mech_long_range_sensor_range(mech),
          mech_scanner_range(mech), mech_heat_sink_count(mech),
          (double)maximum_speed, (double)jump_speed,
          mech_technology_flags(mech));
  for (i = 0; i < NUM_SECTIONS; i++) {
    fprintf(fp, "%d %d %d %d\n", mech_section_armor(mech, i),
            mech_section_internal(mech, i), mech_section_rear_armor(mech, i),
            mech_section_configuration(mech, i));
    for (j = 0; j < NUM_CRITICALS; j++) {
      fprintf(fp, "%d %d %d\n", mech_critical_part_type(mech, i, j),
              mech_critical_data(mech, i, j),
              mech_critical_fire_mode(mech, i, j));
    }
  }
  fprintf(fp, "%d %d\n", mech_class(mech), mech_movement_type(mech));
  fprintf(fp, "%d\n", mech_radio_range(mech));
  if (fclose(fp) != 0) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Unable to finish saving the mech file.");
    return;
  }
  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Saving complete!");
}

/*
 * Template saving routines and logic.
 */
void mechrep_Rsavetemp2(DbRef player, void *data, char *buffer) {
  char *args[1];
  char openfile[512] = {0};

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

  mech_template_registry_clear(mech_context(mech));

  // No template name given.
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "You must specify a template name!");
    return;
  }

  // Anti-twink measure. Don't allow directory saving... yet
  if (strstr(args[0], "/")) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid file name!");
    return;
  }

  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Saving %s", args[0]);
  snprintf(openfile, sizeof(openfile), "%s/",
           btech_context_mech_template_path(mech_context(mech)));
  strlcat(openfile, args[0], sizeof(openfile));

  // Just warn on overweight.
  if (mech_weight_sub(GOD, mech, -1) > (mech_tonnage(mech) * 1024))
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Warning: Template Overweight, see @weight.");

  // I/O or Permissions error.
  if (save_template(player, mech, args[0], openfile) < 0) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Error saving the template file!");
    return;
  }

  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Saving complete!");
} // end mechrep_Rsavetemp2

/*
 * Emits the valid sections when a player tries to setarmor/addsp/reload an
 * invalid section
 */
