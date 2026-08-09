
/* Declares repair damage assessment interfaces. */

#pragma once

#include "equipment_types.h"
#include "section_types.h"
/* Added RESEAL to repair flooded sections
 * -Kipsta
 * 8/4/99
 */

typedef enum RepairDamageType {
  REATTACH,
  REPAIRP,
  REPAIRP_T,
  ENHCRIT_MISC,
  ENHCRIT_FOCUS,
  ENHCRIT_CRYSTAL,
  ENHCRIT_BARREL,
  ENHCRIT_AMMOB,
  ENHCRIT_RANGING,
  ENHCRIT_AMMOM,
  REPAIRG,
  RELOAD,
  FIXARMOR,
  FIXARMOR_R,
  FIXINTERNAL,
  DETACH,
  SCRAPP,
  SCRAPG,
  UNLOAD,
  RESEAL,
  REPLACESUIT,
  NUM_DAMAGE_TYPES
} RepairDamageType;

/* Reattachs / fixints / fixarmors, repair / reload */
enum {
  REPAIR_DAMAGE_CAPACITY = 3 * NUM_SECTIONS + 2 * NUM_SECTIONS * NUM_CRITICALS,
};
