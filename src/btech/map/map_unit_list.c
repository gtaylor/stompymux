#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_obj_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

void map_listmechs(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  Mech *tempMech;
  int i;
  int count = 0;
  char valid[50];
  MechId id;
  char *args[2];
  char *cmds[] = {"MECHS", "OBJS", NULL};
  enum { MECHS, OBJS };

  map = (BattleMap *)data;

  if (mech_parseattributes(buffer, args, 1) == 0) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Supply target type too!");
    return;
  }
  switch (listmatch(cmds, args[0])) {
  case MECHS:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "--- Mechs on Map ---");
    for (i = 0; i < map->first_free; i++) {
      if (map->mechsOnMap[i] != -1) {
        tempMech =
            btech_context_get_mech(map->xcode.context, map->mechsOnMap[i]);
        id = mech_id(tempMech, false);
        if (tempMech)
          strcpy(valid, "Valid Data");
        else
          strcpy(valid, "Invalid Object Data!  Remove this Mech!");
        notify_printf(btech_context_evaluation(map->xcode.context), player,
                      "Mech DB Number: %ld : [%s]\t%s", map->mechsOnMap[i],
                      id.text, valid);
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
                "Invalid argument (%s)!", args[0]);
  return;
}
