/* Declares the BattleTech unit hitloc API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

#include <stdbool.h>

typedef struct HitLocationResult {
  int location;
  bool critical;
  bool rear;
} HitLocationResult;

/* mech.hitloc.c */
int mech_punch_hit_location(Mech *target, int hit_group);
int mech_kick_hit_location(Mech *target, int hit_group);
int mech_battle_suit_hit_location(Mech *mech);
int mech_hit_location_transfer(Mech *mech, int hitloc);
typedef struct CriticalThreshold {
  int armor_percent;
} CriticalThreshold;

bool mech_section_is_crittable(Mech *mech, int loc,
                               CriticalThreshold threshold);
int mech_hit_location(Mech *mech, int hit_group, bool *iscritical,
                      bool *isrear);
HitLocationResult mech_fasa_hit_location(Mech *mech, int hit_group,
                                         HitLocationResult result);
void mech_motive_system_hit(Mech *mech, int w_roll_mod);
HitLocationResult mech_advanced_vehicle_hit_location(Mech *mech, int hit_group,
                                                     HitLocationResult result);
int mech_narc_hit_location(Mech *mech, Mech *hit_mech, bool *t_is_rear_hit);
int mech_target_hit_location(Mech *mech, Mech *target, bool *isrear,
                             bool *iscritical);
int mech_targeting_computer_hit_location(Mech *mech, Mech *target, bool *isrear,
                                         bool *iscritical);
int mech_aimed_hit_location(Mech *mech, Mech *target, bool *isrear,
                            bool *iscritical);
int mech_hit_group(Mech *mech, Mech *target);
