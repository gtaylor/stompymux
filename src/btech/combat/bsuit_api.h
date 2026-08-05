#pragma once

#include "mech.h" /* Temporary transitive compatibility for legacy callers. */
#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;

const char *bsuit_formation_name(const Mech *mech);
const char *bsuit_formation_name_lowercase(const Mech *mech);
void bsuit_recycle_start(Mech *mech, int time);
void bsuit_swarm_stop(Mech *mech, int intentional);
int bsuit_swarmer_count(Mech *mech);
Mech *bsuit_swarmer_find(Mech *mech);
void bsuit_swarmers_stop(BattleMap *map, Mech *mech, int intentional);
int bsuit_has_enemy_swarmers(Mech *mech);
int bsuit_has_friendly_riders(Mech *mech);
void bsuit_swarmers_position_update(BattleMap *map, Mech *mech);
int bsuit_action_validate(Mech *mech, DbRef player);
int bsuit_member_count(const Mech *mech);
int bsuit_target_find(DbRef player, Mech *mech, Mech **target, char *buffer);
int bsuit_jettison_validate(Mech *mech);
void bsuit_swarm(DbRef player, void *data, char *buffer);
void bsuit_attackleg(DbRef player, void *data, char *buffer);
void bsuit_hide(DbRef player, void *data, char *buffer);
void bsuit_pack_jettison(DbRef player, void *data, char *buffer);
