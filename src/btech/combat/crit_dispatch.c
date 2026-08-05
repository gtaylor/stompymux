/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "failures.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_sensor.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tag_api.h"
#include "mech_tech_commands_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"
#include "section_types.h"

void mech_critical_handle(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                          int num) {
  int i;
  int critHit;
  int critType, critData;
  int count, index;
  int critList[NUM_CRITICALS];
  BtechContext *context = mech_context(wounded);
  MechConditionSummary condition = mech_condition_summary(wounded);

  if (condition.combat_safe)
    return;
  if (mech_technology_flags(wounded) & CRITPROOF_TECH)
    return;
  if (mech_class(wounded) == CLASS_MW && btech_random_range(context, 1, 2) == 1)
    return;
  if (mech_class(wounded) != CLASS_MECH &&
      !btech_context_vehicle_critical_mode(context))
    return;
  if (mech_class(wounded) == CLASS_VEH_GROUND ||
      mech_class(wounded) == CLASS_VEH_NAVAL) {
    if (btech_context_uses_advanced_vehicle_criticals(context)) {
      for (i = 0; i < num; i++)
        HandleAdvFasaVehicleCrit(wounded, attacker, LOS, hitloc, num);

      return;
    } else if (!btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        HandleVehicleCrit(wounded, attacker, LOS, hitloc, num);
      return;
    } else if (btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        HandleFasaVehicleCrit(wounded, attacker, LOS, hitloc, num);
      return;
    }
  }
  if (mech_is_dropship(wounded))
    return;
  if (mech_class(wounded) == CLASS_VTOL) {
    if (btech_context_uses_advanced_vtol_criticals(context)) {
      for (i = 0; i < num; i++)
        HandleAdvFasaVehicleCrit(wounded, attacker, LOS, hitloc, num);

      return;
    } else {
      for (i = 0; i < num; i++)
        HandleVTOLCrit(wounded, attacker, LOS, hitloc, num);

      return;
    }
  }
  while (num > 0) {
    count = 0;
    while (count == 0) {
      for (i = 0; i < NUM_CRITICALS; i++) {
        critType = mech_critical_part_type(wounded, hitloc, i);
        if (!mech_critical_is_destroyed(wounded, hitloc, i) &&
            !mech_critical_is_damaged(wounded, hitloc, i) &&
            critType != EMPTY && critType != Special(CASE) &&
            critType != Special(FERRO_FIBROUS) &&
            critType != Special(STEALTH_ARMOR) &&
            critType != Special(HVY_FERRO_FIBROUS) &&
            critType != Special(LT_FERRO_FIBROUS) &&
            critType != Special(ENDO_STEEL) &&
            critType != Special(TRIPLE_STRENGTH_MYOMER) &&
            critType != Special(SUPERCHARGER) && critType != Special(MASC)) {
          critList[count] = i;
          count++;
        }
      }

      if (!count) /* transfer Crit to next location - no longer */
        return;
    }

    index = btech_random_range(context, 0, count - 1);
    critHit = critList[index]; /* This one should be linear */

    critType = mech_critical_part_type(wounded, hitloc, critHit);
    critData = mech_critical_data(wounded, hitloc, critHit);

    if (HandleMechCrit(wounded, attacker, LOS, hitloc, critHit, critType,
                       critData))
      num--;
  }
}
