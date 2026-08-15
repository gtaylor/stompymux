/* Shared types and constants for BattleTech unit APIs. */

#pragma once

#include "equipment_types.h"

typedef struct Mech Mech;

typedef struct MechUnitId {
  char first;
  char second;
} MechUnitId;

typedef struct CriticalSlotReference {
  int section;
  int critical;
} CriticalSlotReference;

typedef struct CriticalSlotLookupResult {
  bool found;
  CriticalSlotReference slot;
} CriticalSlotLookupResult;

typedef struct MechNetworkLink {
  Mech *owner;
  Mech *member;
} MechNetworkLink;

extern const struct WeaponDefinition MECH_WEAPONS[];

constexpr int TELE_ALL = 1;  /* Tele all, not just mortals */
constexpr int TELE_LOUD = 4; /* Loudly teleport */
constexpr int TELE_XP = 8;   /* Lose 1/3 XP */

typedef enum MineTriggerReason : int {
  MINE_COMMAND_DETONATION = 0,
  MINE_STEP = 1, /* Someone steps to a hex */
  MINE_LAND = 2, /* Someone lands in a hex */
  MINE_FALL = 3, /* Someone falls in the hex */
  MINE_DROP = 4, /* Someone drops to ground in the hex */
} MineTriggerReason;

static_assert((MINE_STEP == 1 && MINE_DROP == 4) != 0);

constexpr char WSDUMP_MASK_ER[] =
    "%-24s %2d     %2d           %2d  %2d    %2d  %3d  %3d %2d";
constexpr char WSDUMP_MASK_NOER[] =
    "%-24s %2d     %2d           %2d    %2d     %2d  %3d   %2d";
constexpr char WSDUMP_MASKS_ER[] =
    "[fg=green]Weapon Name             Heat  Damage  Range: Min Short Med Long "
    "Ext "
    "VRT";
constexpr char WSDUMP_MASKS_NOER[] =
    "[fg=green]Weapon Name             Heat  Damage  Range: Min  Short  Med  "
    "Long  "
    "VRT";

constexpr char WDUMP_MASK[] =
    "%-24s %2d     %2d           %2d  %2d    %2d  %3d  %2d  %2d %d";
constexpr char WDUMP_MASKS[] =
    "[fg=green]Weapon Name             Heat  Damage  Range: Min Short Med Long "
    "VRT C "
    " ApT";
enum { LOS_TRACE_CAPACITY = 4000 };

typedef struct LosTracePoint {
  int x;
  int y;
} LosTracePoint;

typedef struct LosTrace {
  LosTracePoint points[LOS_TRACE_CAPACITY];
} LosTrace;
