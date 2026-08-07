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
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "section_types.h"

int mech_head_hit_modify(int hitGroup, Mech *mech);
int mech_punch_hit_location(Mech *target, int hitGroup);
int mech_kick_hit_location(Mech *target, int hitGroup);
int mech_battle_suit_hit_location(Mech *mech);
int mech_hit_location_transfer(Mech *mech, int hitloc);
int mech_section_is_crittable(Mech *mech, int loc, int tres);
int mech_spheroid_rear_section(const Mech *mech, int section);
void mech_motive_system_hit(Mech *mech, int wRollMod);
int mech_fasa_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear);
int fasa_mech_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear, int roll);
int fasa_ground_hit_location(Mech *mech, int hitGroup, int *iscritical,
                             int *isrear, int roll);
int fasa_aerospace_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                int *isrear, int roll);
int fasa_vtol_naval_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                 int *isrear, int roll);
int mech_advanced_vehicle_hit_location(Mech *mech, int hitGroup,
                                       int *iscritical, int *isrear);
int mech_critproof_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                int *isrear);
int mech_hit_location(Mech *mech, int hitGroup, int *iscritical, int *isrear);
int find_swarm_hit_location(BtechContext *context, int *iscritical,
                            int *isrear);
