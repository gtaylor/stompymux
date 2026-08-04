/* Shared implementation dependencies for hit-location tables. */
#pragma once

#include <string.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "crit_api.h"
#include "map_obj_api.h"
#include "mech.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"

#define CHECK_ZERO_LOC(mech, a, b) (GetSectInt(mech, a) > 0 ? a : b)

int ModifyHeadHit(int hitGroup, Mech *mech);
int FindPunchLocation(Mech *target, int hitGroup);
int FindKickLocation(Mech *target, int hitGroup);
int get_bsuit_hitloc(Mech *mech);
int TransferTarget(Mech *mech, int hitloc);
int crittable(Mech *mech, int loc, int tres);
void DoMotiveSystemHit(Mech *mech, int wRollMod);
int FindFasaHitLocation(Mech *mech, int hitGroup, int *iscritical, int *isrear);
int fasa_mech_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear, int roll);
int fasa_ground_hit_location(Mech *mech, int hitGroup, int *iscritical,
                             int *isrear, int roll);
int fasa_aerospace_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                int *isrear, int roll);
int fasa_vtol_naval_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                 int *isrear, int roll);
int FindAdvFasaVehicleHitLocation(Mech *mech, int hitGroup, int *iscritical,
                                  int *isrear);
int FindHitLocation_CritProof(Mech *mech, int hitGroup, int *iscritical,
                              int *isrear);
int FindHitLocation(Mech *mech, int hitGroup, int *iscritical, int *isrear);
int find_swarm_hit_location(BtechContext *context, int *iscritical,
                            int *isrear);
