/* Declares the BattleTech unit hitloc API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

/* mech.hitloc.c */
int mech_punch_hit_location(Mech *target, int hitGroup);
int mech_kick_hit_location(Mech *target, int hitGroup);
int mech_battle_suit_hit_location(Mech *mech);
int mech_hit_location_transfer(Mech *mech, int hitloc);
int mech_section_is_crittable(Mech *m, int loc, int tres);
int mech_hit_location(Mech *mech, int hitGroup, int *iscritical, int *isrear);
int mech_fasa_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear);
void mech_motive_system_hit(Mech *mech, int wRollMod);
int mech_advanced_vehicle_hit_location(Mech *mech, int hitGroup,
                                       int *iscritical, int *isrear);
int mech_narc_hit_location(Mech *mech, Mech *hitMech, int *tIsRearHit);
int mech_target_hit_location(Mech *mech, Mech *target, int *isrear,
                             int *iscritical);
int mech_targeting_computer_hit_location(Mech *mech, Mech *target, int *isrear,
                                         int *iscritical);
int mech_aimed_hit_location(Mech *mech, Mech *target, int *isrear,
                            int *iscritical);
int mech_hit_group(Mech *mech, Mech *target);
