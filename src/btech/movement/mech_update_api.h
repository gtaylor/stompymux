
/*
   p.mech.update.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:16 CET 1999 from mech.update.c */

#pragma once

#include "mux/server/platform.h"

/* mech.update.c */
bool mech_fire_hazard_resolve(Mech *mech);
int bridge_w_elevation(Mech *mech);
void bridge_set_elevation(Mech *mech);
bool dropship_notification_is_due(Mech *mech);
void move_mech(Mech *mech);
void mech_naval_altitude_check(Mech *mech, int previous_z);
void mech_vtol_altitude_check(Mech *mech);
void mech_heading_update(Mech *mech);
float mech_terrain_speed(Mech *mech, float current_speed, float maximum_speed,
                         int terrain, int elevation);
void mech_speed_update(Mech *mech);
int mech_overheat_to_hit_modifier(const Mech *mech);
void mech_overheat_handle(Mech *mech);
void mech_heat_update(Mech *mech);
int mech_weapon_recycle_update(Mech *mech);
int mech_skid_modifier(float speed);
void NewHexEntered(Mech *mech, BattleMap *mech_map, float deltax, float deltay,
                   int last_z);
void mech_damage_stagger_check(Mech *wounded);
void mech_piloting_update(Mech *mech);
void mech_turret_autoturn_update(Mech *mech);
void mech_update(DbRef key, void *data);
