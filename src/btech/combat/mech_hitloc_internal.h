/* Shared implementation dependencies for hit-location tables. */
#pragma once

#include <string.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "crit_api.h"
#include "map_obj_api.h"
#include "mech_api_types.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "section_types.h"

int mech_head_hit_modify(int hit_group, Mech *mech);
int mech_spheroid_rear_section(const Mech *mech, int section);
static inline HitLocationResult hit_location_result_at(HitLocationResult result,
                                                       int location) {
  result.location = location;
  return result;
}

HitLocationResult fasa_mech_hit_location(Mech *mech, int hit_group,
                                         HitLocationResult result, int roll);
HitLocationResult fasa_ground_hit_location(Mech *mech, int hit_group,
                                           HitLocationResult result, int roll);
HitLocationResult fasa_aerospace_hit_location(Mech *mech, int hit_group,
                                              HitLocationResult result,
                                              int roll);
HitLocationResult fasa_vtol_naval_hit_location(Mech *mech, int hit_group,
                                               HitLocationResult result,
                                               int roll);
int mech_critproof_hit_location(Mech *mech, int hit_group, int *iscritical);
HitLocationResult find_swarm_hit_location(BtechContext *context);
