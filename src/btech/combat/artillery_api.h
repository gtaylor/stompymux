/* Declares the BattleTech artillery API. */

#pragma once

#include "map_coordinates.h"
#include "mux/server/platform.h"

typedef struct ArtilleryShotRequest {
  Mech *mech;
  MapHexPosition target;
  int weapon_index;
  int weapon_mode;
  bool hit;
} ArtilleryShotRequest;

typedef struct BlastDamage {
  int total;
  int hit_size;
  int heat;
} BlastDamage;

typedef struct BlastMessages {
  const char *target;
  const char *observers;
} BlastMessages;

typedef struct BlastSafety {
  int above;
  int below;
  bool underwater;
} BlastSafety;

typedef struct BlastRealHexRequest {
  BattleMap *map;
  BlastDamage damage;
  MapRealPosition impact;
  MapRealPosition source;
  BlastMessages messages;
  int hit_table;
  BlastSafety safety;
} BlastRealHexRequest;

typedef struct BlastHexRequest {
  BattleMap *map;
  BlastDamage damage;
  MapHexPosition impact;
  MapHexPosition source;
  BlastMessages messages;
  int hit_table;
  BlastSafety safety;
} BlastHexRequest;

typedef struct BlastRealAreaRequest {
  BlastRealHexRequest center;
  BlastMessages neighbor_messages;
  int neighbor_radius;
} BlastRealAreaRequest;

typedef struct BlastAreaRequest {
  BlastHexRequest center;
  BlastMessages neighbor_messages;
  int neighbor_radius;
} BlastAreaRequest;

/* artillery.c */
int artillery_round_flight_time(float fx, float fy, float tx, float ty);
void artillery_shoot(const ArtilleryShotRequest *request);
void blast_hit_real_hex(const BlastRealHexRequest *request);
void blast_hit_hex(const BlastHexRequest *request);
void blast_hit_real_area(const BlastRealAreaRequest *request);
void blast_hit_area(const BlastAreaRequest *request);
void artillery_friendly_adjustment(DbRef mechnum, BattleMap *map, int x, int y);
