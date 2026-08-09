#pragma once
#include "btech_api.h"   // IWYU pragma: keep
#include "btech_event.h" // IWYU pragma: keep
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_lifecycle.h"
#include "mux/objects/powers.h"       // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "map_obj_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_notify_api.h"
#include "mux/commands/command_handlers.h"
#include "mux/objects/attrs.h"
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/player.h"
#include "special_object.h"
#include "weapon_settings.h"

#include "coolmenu.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_progress_api.h"
#include "mycool.h"

#include "btechstats.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_partnames_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
int character_value_by_code(BtechContext *context, DbRef player, int code);
void character_stats_clear(PSTATS *stats);
void character_stats_retrieve(BtechContext *context, DbRef player, int modes,
                              PSTATS *stats);
void character_stats_store(BtechContext *context, DbRef player, PSTATS *stats,
                           int modes);
bool character_state_validate_all(BtechContext *context);
int character_xp_to_next_level(BtechContext *context, DbRef target, int code);
PSTATS *character_stats_create(void);
const CharacterValue *character_value_definition(int code);
const char *character_value_type_name(int type);
void character_value_xp_threshold_set(int code, int threshold);
unsigned char character_stats_value_get(const PSTATS *stats, int code);
void character_stats_value_set(PSTATS *stats, int code, int value);
int character_stats_xp_get(const PSTATS *stats, int code);
void character_stats_xp_set(PSTATS *stats, int code, int value);
time_t character_stats_last_use_get(const PSTATS *stats, int code);
void character_stats_last_use_set(PSTATS *stats, int code, time_t value);
int char_getstatvalue(PSTATS *stats, const char *name);
void char_setstatvalue(PSTATS *stats, const char *name, int value);
int figure_xp_bonus(BtechContext *context, DbRef player, PSTATS *stats,
                    int code);
void character_value_set_by_code(BtechContext *context, DbRef player, int code,
                                 int value);

enum { MAX_PLAYERS_ON = 10000 };
