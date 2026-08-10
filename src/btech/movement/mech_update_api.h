
#pragma once

#include "map_coordinates.h"
#include "mux/server/platform.h"

bool mech_fire_hazard_resolve(Mech *mech);
int bridge_w_elevation(Mech *mech);
void bridge_set_elevation(Mech *mech);
bool dropship_notification_is_due(Mech *mech);
void dropship_notification_broadcast(Mech *mech, const char *message);
void dropship_notification_broadcast_if_due(Mech *mech, const char *message);
void mech_movement_update(Mech *mech);
void mech_naval_altitude_check(Mech *mech, int previous_z);
void mech_vtol_altitude_check(Mech *mech);
void mech_heading_update(Mech *mech);
typedef struct MechTerrainSpeedRequest {
  Mech *mech;
  float current_speed;
  float maximum_speed;
  int terrain;
  int elevation;
} MechTerrainSpeedRequest;
float mech_terrain_speed(const MechTerrainSpeedRequest *request);
void mech_speed_update(Mech *mech);
int mech_overheat_to_hit_modifier(const Mech *mech);
void mech_overheat_handle(Mech *mech);
void mech_heat_update(Mech *mech);
int mech_weapon_recycle_update(Mech *mech);
int mech_skid_modifier(float speed);
typedef struct MechHexEntryRequest {
  Mech *mech;
  BattleMap *map;
  MapRealPosition delta;
  int previous_z;
} MechHexEntryRequest;
void mech_hex_entry_resolve(const MechHexEntryRequest *request);
void mech_damage_stagger_check(Mech *wounded);
void mech_piloting_update(Mech *mech);
void mech_turret_autoturn_update(Mech *mech);
void mech_update(DbRef key, void *data);
