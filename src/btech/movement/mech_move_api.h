/* Declares the BattleTech unit move API. */

#pragma once

#include "mux/server/platform.h"

/* mech.move.c */
const char *mech_lateral_description(Mech *mech);
bool mech_lateral_mode_details(int mode, const char **description, int *offset);
void mech_lateral(DbRef player, void *data, char *buffer);
void mech_turnmode(DbRef player, void *data, char *buffer);
void mech_bootlegger(DbRef player, void *data, char *buffer);
void mech_eta(DbRef player, void *data, char *buffer);
float mech_cargo_maximum_speed(Mech *mech, float maximum_speed);
float mech_effective_maximum_speed(Mech *mech);
int mech_jump_speed_mp_for_map(const Mech *mech, const BattleMap *map);
float mech_jump_speed_for_map(const Mech *mech, const BattleMap *map);
void mech_drop(DbRef player, void *data, const char *buffer);
void mech_stand(DbRef player, void *data, char *buffer);
void mech_stand_empty(DbRef player, void *data);
void mech_land(DbRef player, void *data, char *buffer);
void mech_heading(DbRef player, void *data, char *buffer);
void mech_turret(DbRef player, void *data, char *buffer);
void mech_rotatetorso(DbRef player, void *data, char *buffer);
void mech_speed(DbRef player, void *data, char *buffer);
void mech_vertical(DbRef player, void *data, char *buffer);
void mech_thrash(DbRef player, void *data, char *buffer);
void mech_jump(DbRef player, void *data, char *buffer);
void mech_hulldown(DbRef player, void *data, char *buffer);
#ifdef BT_MOVEMENT_MODES
void mech_sprint(DbRef player, void *data, char *buffer);
void mech_evade(DbRef player, void *data, char *buffer);
void mech_dodge(DbRef player, void *date, char *buffer);
#endif
int mech_drop_surface_elevation(Mech *mech);
void mech_drop_surface_set(Mech *mech, bool check_ice);
int mech_drop_height_above_surface(Mech *mech);
int mech_upper_surface_elevation(Mech *mech);
int mech_lower_surface_elevation(Mech *mech);
int mech_height_above_surface(Mech *mech);
void mech_jump_land(Mech *mech);
void mech_flood_section(Mech *mech, int section, int elevation);
void mech_flood(Mech *mech);
void mech_fall(Mech *mech, int levels, int seemsg);
typedef enum MechDominoMode {
  MECH_DOMINO_GROUND,
  MECH_DOMINO_JUMP,
  MECH_DOMINO_FALL,
} MechDominoMode;

int battle_map_mech_count_in_hex(BattleMap *map, int x, int y, int friendly,
                                 int team);
int mech_domino_resolve(Mech *mech, MechDominoMode mode);
