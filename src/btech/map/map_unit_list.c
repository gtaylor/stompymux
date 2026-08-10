#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_units_api.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

void map_listmechs(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  Mech *tempMech;
  int i;
  int count = 0;
  char valid[50];
  MechId id;
  char *args[2];
  const char *const cmds[] = {"MECHS", "OBJS", nullptr};
  enum { MECHS, OBJS };

  map = (BattleMap *)data;

  if (mech_parseattributes(buffer, args, 1) == 0) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Supply target type too!");
    return;
  }
  char **argument_slot =
      (char **)checked_storage_at((void *)args, 2, sizeof(*args), 0);
  switch (listmatch(cmds, 2, *argument_slot)) {
  case MECHS:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "--- Mechs on Map ---");
    for (i = 0; i < battle_map_unit_count(map); i++) {
      const DbRef unit_dbref = battle_map_unit_dbref(map, i);
      if (unit_dbref != -1) {
        tempMech = btech_context_get_mech(map->xcode.context, unit_dbref);
        if (tempMech) {
          id = mech_id(tempMech, false);
          strcpy(valid, "Valid Data");
        } else {
          id = (MechId){0};
          strcpy(valid, "Invalid Object Data!  Remove this Mech!");
        }
        notify_printf(btech_context_evaluation(map->xcode.context), player,
                      "Mech DB Number: %ld : [%s]\t%s", unit_dbref, id.text,
                      valid);
        count++;
      }
    }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d Mechs On Map", count);
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d positions open", MAX_MECHS_PER_MAP - count);
    if (count != map->first_free)
      notify_printf(btech_context_evaluation(map->xcode.context), player,
                    "%d is first free slot, according to db.", map->first_free);
    return;
    break;
  case OBJS:
    list_mapobjs(player, map);
    return;
    break;
  }
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Invalid argument (%s)!", *argument_slot);
  return;
}
