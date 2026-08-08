
/*
   p.mech.lostracer.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Sun Jun  7 18:16:48 EEST 1998 from los_trace.c */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;

int trace_los(BattleMap *map, int ax, int ay, int bx, int by, LosTrace *trace);
const LosTracePoint *los_trace_point_at(const LosTrace *trace, int index);
