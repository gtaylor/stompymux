/* Declares the BattleTech unit restrict API. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"
#include "special_object.h"

typedef enum MechMapSetResult : int {
  MECH_MAP_SET_OK,
  MECH_MAP_SET_INVALID_DESTINATION,
  MECH_MAP_SET_INVALID_CURRENT_MAP,
  MECH_MAP_SET_FULL,
} MechMapSetResult;

typedef struct MechPositionSetRequest {
  Mech *mech;
  int x;
  int y;
  int z;
  bool has_z;
} MechPositionSetRequest;

typedef struct MechMapSetBatchRequest {
  Mech *const *mechs;
  size_t count;
  DbRef map;
} MechMapSetBatchRequest;

typedef struct MechMapPositionRequest {
  BtechContext *context;
  DbRef map;
  int x;
  int y;
} MechMapPositionRequest;

/* mech.restrict.c */
void clear_mech_from_los(Mech *mech);
[[nodiscard]] MechMapSetResult mech_map_index_set(Mech *mech, DbRef map,
                                                  const char *preferred_id);
[[nodiscard]] MechMapSetResult
mech_map_index_preflight_batch(const MechMapSetBatchRequest *request);
[[nodiscard]] MechMapSetResult
mech_map_index_set_batch(const MechMapSetBatchRequest *request);
[[nodiscard]] bool
mech_map_position_is_valid(const MechMapPositionRequest *request);
[[nodiscard]] bool mech_position_set(const MechPositionSetRequest *request);
void mech_rsetxy(DbRef player, Mech *mech, char *buffer);
void mech_rsetmapindex(DbRef player, Mech *mech, char *buffer);
void mech_rsetteam(DbRef player, Mech *mech, char *buffer);
void newfreemech(DbRef key, void **data,
                 BtechSpecialLifecycleOperation selector);
