/* Declares the BattleTech unit lostracer API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;

int trace_los(BattleMap *map, int ax, int ay, int bx, int by, LosTrace *trace);
const LosTracePoint *los_trace_point_at(const LosTrace *trace, int index);
