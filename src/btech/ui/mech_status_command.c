#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_status_api.h"
#include "mech_status_render_internal.h"

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

static int displayed_speed(float speed) { return clamp_float_to_int(speed); }

static char status_option_character(const char *options, size_t length,
                                    size_t position) {
  return position < length ? *checked_string_suffix(options, position) : '\0';
}

/* Status commands! */
void mech_status(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  int doweap = 0;
  int doinfo = 0;
  int doarmor = 0;
  int doshort = 0;
  int doheat = 0;
  int i;
  int usex = 0;
  bool weird = false;
  char buf[LBUF_SIZE] = {0};
  char weird_buffer[LBUF_SIZE] = {0};

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  if (!buffer || !strlen(buffer)) {
    // No arguments, we'll go with our default 'status' output.
    doweap = doinfo = doarmor = doheat = 1;
  } else {
    // Argument provided, only show certain parts.
    const size_t OPTION_LENGTH = strlen(buffer);
    for (size_t position = 0; position < OPTION_LENGTH; position++) {
      switch (ascii_to_upper(
          status_option_character(buffer, OPTION_LENGTH, position))) {
      case 'R':
        doweap = doinfo = doarmor = doheat = usex = 1;
        break;
      case 'A':
        // Armor status
        if (ascii_to_upper(status_option_character(buffer, OPTION_LENGTH,
                                                   position + 1)) == 'R')
          while (status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     '\0' &&
                 status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     ' ')
            position++;
        doarmor = 1;
        break;
      case 'I':
        // Speed/Heading/Heat
        doinfo = 1;
        if (ascii_to_upper(status_option_character(buffer, OPTION_LENGTH,
                                                   position + 1)) == 'N')
          while (status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     '\0' &&
                 status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     ' ')
            position++;
        break;
      case 'W':
        // Weapons list.
        doweap = 1;
        if (ascii_to_upper(status_option_character(buffer, OPTION_LENGTH,
                                                   position + 1)) == 'E')
          while (status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     '\0' &&
                 status_option_character(buffer, OPTION_LENGTH, position + 1) !=
                     ' ')
            position++;
        break;
      case 'N':
        // Really weird status display.
        weird = true;
        break;
      case 'S':
        // Very short one-line status.
        doshort = 1;
        break;
      case 'H':
        // Just the heat bar.
        doheat = 1;
        break;
      }
    }
  }

  // Very short one-line status.
  if (doshort) {
    print_short_info(evaluation, player, mech);
    return;
  }

  // Really weird status display.
  if (weird) {
    (void)snprintf(buf, sizeof(buf), "%s %s %d %d/%d/%d %d ",
                   mech_model_reference(mech), mech_model_name(mech),
                   mech_tonnage(mech),
                   displayed_speed(mech_maximum_speed(mech) / MP1) * 2 / 3,
                   displayed_speed(mech_maximum_speed(mech) / MP1),
                   displayed_speed(mech_jump_speed(mech) / MP1),
                   displayed_speed(mech_active_heat_sinks(mech)));
    memcpy(weird_buffer, buf, sizeof(weird_buffer));

  } else if (!doheat || (doarmor | doinfo | doweap)) {
    print_generic_status(evaluation, player, mech, usex != 0);
  }

  // Show our armor diagram.
  if (doarmor) {
    if (!weird) {
      print_armor_status(evaluation, player, mech, 1);
      mecha_notify(evaluation, player, " ");
    } else {
      for (i = 0; i < NUM_SECTIONS; i++)
        if (mech_section_original_armor(mech, i)) {
          if (mech_section_original_rear_armor(mech, i))
            append_status(buf, sizeof(buf), "%d|%d|%d ",
                          mech_section_original_armor(mech, i),
                          mech_section_original_internal(mech, i),
                          mech_section_original_rear_armor(mech, i));
          else
            append_status(buf, sizeof(buf), "%d|%d ",
                          mech_section_original_armor(mech, i),
                          mech_section_original_internal(mech, i));
        }
    }
  }

  // Standard heat/heading/dive/etc.
  if (doinfo && !weird) {
    print_info_status(evaluation, player, mech, 1);
    // mecha_notify(evaluation, player, " ");
  }

  // Show our heat bar by itself.
  if (!doinfo && doheat && mech_uses_heat(mech)) {
    print_heat_bar(evaluation, player, mech);
  }

  // Weapons readout.
  if (doweap)
    print_weapon_status(evaluation, mech, player, weird, weird_buffer,
                        sizeof(weird_buffer));

  // Really strange, short status info.
  if (weird)
    mecha_notify(evaluation, player, weird_buffer);
}
