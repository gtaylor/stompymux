/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#pragma once

#include "equipment_types.h"
#include "mux/objects/db.h"

typedef struct Mech Mech;

typedef struct MechUnitId {
  char first;
  char second;
} MechUnitId;

extern const struct WeaponDefinition MechWeapons[];

constexpr int TELE_ALL = 1;  /* Tele all, not just mortals */
constexpr int TELE_LOUD = 4; /* Loudly teleport */
constexpr int TELE_XP = 8;   /* Lose 1/3 XP */

constexpr int MINE_STEP = 1; /* Someone steps to a hex */
constexpr int MINE_LAND = 2; /* Someone lands in a hex */
constexpr int MINE_FALL = 3; /* Someone falls in the hex */
constexpr int MINE_DROP = 4; /* Someone drops to ground in the hex */

#ifndef ECMD
#define ECMD(a) extern void a(DbRef player, void *data, char *buffer)
#endif

#define A_MECHREF A_MECHTYPE
#define WSDUMP_MASK_ER                                                         \
  "%-24s %2d     %2d           %2d  %2d    %2d  %3d  %3d %2d"
#define WSDUMP_MASK_NOER                                                       \
  "%-24s %2d     %2d           %2d    %2d     %2d  %3d   %2d"
#define WSDUMP_MASKS_ER                                                        \
  "[fg=green]Weapon Name             Heat  Damage  Range: Min Short Med Long " \
  "Ext "                                                                       \
  "VRT"
#define WSDUMP_MASKS_NOER                                                      \
  "[fg=green]Weapon Name             Heat  Damage  Range: Min  Short  Med  "   \
  "Long  "                                                                     \
  "VRT"

#define WDUMP_MASK                                                             \
  "%-24s %2d     %2d           %2d  %2d    %2d  %3d  %2d  %2d %d"
#define WDUMP_MASKS                                                            \
  "[fg=green]Weapon Name             Heat  Damage  Range: Min Short Med Long " \
  "VRT C "                                                                     \
  " ApT"
enum { LOS_TRACE_CAPACITY = 4000 };

typedef struct LosTracePoint {
  int x;
  int y;
} LosTracePoint;

typedef struct LosTrace {
  LosTracePoint points[LOS_TRACE_CAPACITY];
} LosTrace;
