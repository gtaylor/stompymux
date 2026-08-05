
/* p.mech.damage.h */

#pragma once

#include "mux/server/platform.h"

int cause_armordamage(Mech *wounded, Mech *attacker, int LOS, int attackPilot,
                      int isrear, int iscritical, int hitloc, int damage,
                      int *crits, int wWeapIndx, int wAmmoMode);
int cause_internaldamage(Mech *wounded, Mech *attacker, int LOS,
                         int attackPilot, int isrear, int hitloc, int intDamage,
                         int weapindx, int *crits);
void DamageMech(Mech *wounded, Mech *attacker, int LOS, int attackPilot,
                int hitloc, int isrear, int iscritical, int damage,
                int intDamage, int cause, int bth, int wWeapIndx, int wAmmoMode,
                int tIgnoreSwarmers);
void mech_weapon_destroy(Mech *wounded, int hitloc, int type,
                         int start_critical, int critical_count,
                         int total_criticals);
int mech_weapon_count_in_section(Mech *mech, int section);
int mech_weapon_index_in_section(Mech *mech, int section, int ordinal);
void mech_weapon_destroy_random(Mech *mech, int section);
void mech_heat_sink_destroy(Mech *mech, int section);
void mech_section_destroy(Mech *wounded, Mech *attacker, int line_of_sight,
                          int section);
char *mech_armor_status_set_value(Mech *mech, char *section, char *armor_type,
                                  char *value);
int mech_damage_apply_clusters(DbRef player, Mech *mech, int total_damage,
                               int cluster_size, int direction, int critical,
                               char *mech_message, char *broadcast_message);
void mech_damage(DbRef player, Mech *mech, char *buffer);
void mech_damage_section(DbRef player, Mech *mech, char *buffer);
