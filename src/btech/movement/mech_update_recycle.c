/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_api.h"
#include "weapon_catalogue_api.h"

#include "btech/context.h"
#include "failures.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

static bool mech_section_recycles(const Mech *mech) {
  int unit_class = mech_class(mech);
  return unit_class == CLASS_MECH || unit_class == CLASS_BSUIT ||
         unit_class == CLASS_VEH_GROUND || unit_class == CLASS_VTOL;
}

int mech_weapon_recycle_update(Mech *mech) {
  int criticals[MAX_WEAPS_SECTION];
  unsigned char weapon_types[MAX_WEAPS_SECTION];
  unsigned char weapon_data[MAX_WEAPS_SECTION];
  char location[20];
  BtechContext *context = mech_context(mech);
  int tick = btech_context_event_tick(context);
  int diff = tick - mech_last_weapon_recycle_tick(mech);
  int lowest = 0;

  if (diff < 1) {
    if (diff < 0)
      mech_last_weapon_recycle_tick_set(mech, tick);
    return 1;
  }
  mech_last_weapon_recycle_tick_set(mech, tick);

  if (!mech_is_started(mech) || mech_is_destroyed(mech))
    return 0;

  btech_context_combat_arcs_override_set(context, 1);
  for (int section = 0; section < NUM_SECTIONS; section++) {
    int count = FindWeapons_Advanced(mech, section, weapon_types, weapon_data,
                                     criticals, 1);
    for (int weapon = 0; weapon < count; weapon++) {
      int critical = criticals[weapon];
      if (!mech_weapon_is_recycling_at(mech, section, critical))
        continue;

      if (mech_critical_temporary_failure(mech, section, critical) ==
              FAIL_DESTROYED ||
          mech_section_is_destroyed(mech, section))
        mech_critical_data_set(mech, section, critical, 0);
      if (diff >= mech_critical_data(mech, section, critical)) {
        mech_critical_data_set(mech, section, critical, 0);
        /* ROCKET_FIRED intentionally selects an empty notification. */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-zero-length"
#endif
        mech_printf(
            mech, MECHSTARTED,
            mech_class(mech) == CLASS_MW
                ? "[fg=green]You are ready to attack again with %s.[reset]"
            : mech_critical_temporary_failure(mech, section, critical) != 0
                ? "[fg=green]%s is operational again.[reset]"
            : (mech_critical_fire_mode(mech, section, critical) & ROCKET_FIRED)
                ? ""
                : "[fg=green]%s finished recycling.[reset]",
            &weapon_catalogue_name(weapon_types[weapon])[3]);
#ifdef __clang__
#pragma clang diagnostic pop
#endif
        mech_critical_temporary_failure_set(mech, section, critical, 0);
      } else if (mech_critical_temporary_failure(mech, section, critical) !=
                 FAIL_DESTROYED) {
        int remaining = mech_critical_data(mech, section, critical) - diff;
        mech_critical_data_set(mech, section, critical, remaining);
        if (remaining < lowest || !lowest)
          lowest = remaining;
      }
    }

    int section_recycle = mech_section_recycle_ticks(mech, section);
    if (!section_recycle || !mech_section_recycles(mech))
      continue;

    if (diff >= section_recycle && !mech_section_is_destroyed(mech, section)) {
      mech_section_recycle_ticks_set(mech, section, 0);
      ArmorStringFromIndex(section, location, mech_class(mech),
                           mech_movement_type(mech));
      mech_printf(mech, MECHSTARTED,
                  "[fg=green]%s%s has finished its previous action.[reset]",
                  mech_class(mech) == CLASS_BSUIT ? "" : "Your ", location);
    } else {
      section_recycle -= diff;
      mech_section_recycle_ticks_set(mech, section, section_recycle);
      if (section_recycle < lowest || !lowest)
        lowest = section_recycle;
    }
  }
  btech_context_combat_arcs_override_set(context, 0);
  return lowest;
}
