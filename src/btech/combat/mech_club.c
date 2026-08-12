#include "btconfig.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include <stdio.h>
bool mech_club_location_is_usable(Mech *mech, int section, bool emit_failure) {
  int t_can_grab = 1;
  char buf[100] = {0};
  char location[20] = {0};

  armor_string_from_index(section, location, mech_class(mech),
                          mech_movement_type(mech));

  if (mech_section_is_destroyed(mech, section)) {
    (void)snprintf(buf, sizeof(buf), "Your %s is destroyed.", location);
    t_can_grab = 0;
  } else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = mech,
                 .slot = {.section = section, .critical = 3},
                 .special = HAND_OR_FOOT_ACTUATOR})) {
    (void)snprintf(buf, sizeof(buf),
                   "Your %s's hand actuator is destroyed or missing.",
                   location);
    t_can_grab = 0;
  } else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = mech,
                 .slot = {.section = section, .critical = 0},
                 .special = SHOULDER_OR_HIP})) {
    (void)snprintf(buf, sizeof(buf),
                   "Your %s's shoulder actuator is destroyed or missing.",
                   location);
    t_can_grab = 0;
  } else if (mech_section_has_recycling_weapon(mech, section)) {
    (void)snprintf(buf, sizeof(buf),
                   "Your %s is still recovering from it's last attack.",
                   location);
    t_can_grab = 0;
  }

  if (!t_can_grab && emit_failure)
    mech_notify(mech, MECHALL, buf);

  return t_can_grab;
}

/*
 * Handles the grabbing of a club.
 */
void mech_grabclub(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int wc_args = 0;
  int location = 0;
  char *args[1];
  char locname[20];

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  wc_args = mech_parseattributes(buffer, args, 1);

  // If we grabclub -, we're attempting to drop it.
  if (wc_args >= 1 &&
      ascii_to_upper(*checked_string_suffix(args[0], 0)) == '-') {
    if (mech_section_carries_club(mech, LARM) ||
        mech_section_carries_club(mech, RARM)) {
      mech_drop_club(mech);
    } else {
      mech_notify(mech, MECHALL, "You aren't currently carrying a club.");
    }
    return;
  } // end if() - Check to drop club.

  if (mech_is_quad(mech)) {
    mech_notify(mech, MECHALL, "Quads can't carry a club.");
    return;
  }
  if (mech_condition_summary(mech).fallen) {
    mech_notify(mech, MECHALL,
                "You can't grab a club while lying flat on your face.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL, "You can't grab a club while jumping!");
    return;
  }
  if (mech_cocoon_integrity(mech)) {
    mech_notify(mech, MECHALL, "Your rapid descent prevents that.");
    return;
  }
  if (mech_event_count(mech, EVENT_UNJAM_AMMO)) {
    mech_notify(mech, MECHALL, "You are too busy unjamming a weapon!");
    return;
  }
  if (mech_event_count(mech, EVENT_REMOVE_PODS)) {
    mech_notify(mech, MECHALL, "You are too busy removing iNARC pods!");
    return;
  }

  // If they already have a physical weapon, disallow the grabbing of a club.
  if (have_axe(mech, LARM) || have_axe(mech, RARM)) {
    mech_notify(mech, MECHALL, "You can not grab a club if you carry an axe.");
    return;
  }
  if (have_sword(mech, LARM) || have_sword(mech, RARM)) {
    mech_notify(mech, MECHALL, "You can not grab a club if you carry a sword.");
    return;
  }
  if (have_mace(mech, LARM) || have_mace(mech, RARM)) {
    mech_notify(mech, MECHALL, "You can not grab a club if you carry an mace.");
    return;
  }

  if (wc_args == 0) {
    if (mech_club_location_is_usable(mech, LARM, false)) {
      location = LARM;
    } else if (mech_club_location_is_usable(mech, RARM, false)) {
      location = RARM;
    } else {
      mech_notify(mech, MECHALL,
                  "You don't have a free arm with a working hand actuator!");
      return;
    }
  } else {

    // Figure out which arm to use.
    switch (ascii_to_upper(*checked_string_suffix(args[0], 0))) {
    case 'R':
      location = RARM;
      break;
    case 'L':
      location = LARM;
      break;
    default:
      mech_notify(mech, MECHALL, "Invalid option for 'grabclub'");
      return;
    } // end switch() - Determine location.

    // see if we have actuators and a working arm.
    if (!mech_club_location_is_usable(mech, location, true))
      return;
  }

  if (mech_section_carries_club(mech, LARM) ||
      mech_section_carries_club(mech, RARM)) {
    mech_notify(mech, MECHALL, "You're already carrying a club.");
    return;
  }
  if (mech_real_terrain_get(mech) != HEAVY_FOREST &&
      mech_real_terrain_get(mech) != LIGHT_FOREST) {
    mech_notify(mech, MECHALL,
                "There don't appear to be any trees within grabbing distance.");
    return;
  }

  armor_string_from_index(location, locname, mech_class(mech),
                          mech_movement_type(mech));

  mech_los_broadcast(mech, "reaches down and yanks a tree out of the ground!");
  mech_printf(mech, MECHALL,
              "You reach down and yank a tree out of the ground with your %s.",
              locname);

  // Grabbing a club sets a flag and recycles the arm used.
  mech_section_special_add(mech, location, CARRYING_CLUB);
  mech_set_recycle_limb(mech, location, PHYSICAL_RECYCLE_TIME);
} // end mech_grabclub()
