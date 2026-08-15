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
  Mech *temp_mech;
  int i;
  int count = 0;
  char valid[50];
  MechId id;
  char *args[2];
  const char *const CMDS[] = {"MECHS", "OBJS", nullptr};
  enum { MECHS, OBJS };

  map = (BattleMap *)data;

  if (mech_parseattributes(buffer, args, 1) == 0) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Supply target type too!");
    return;
  }
  char **argument_slot =
      (char **)checked_storage_at((void *)args, 2, sizeof(*args), 0);
  switch (listmatch(CMDS, 2, *argument_slot)) {
  case MECHS:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "--- Mechs on Map ---");
    for (i = 0; i < battle_map_unit_count(map); i++) {
      const DbRef UNIT_DBREF = battle_map_unit_dbref(map, i);
      if (UNIT_DBREF != -1) {
        temp_mech = btech_context_get_mech(map->xcode.context, UNIT_DBREF);
        if (temp_mech) {
          id = mech_id(temp_mech, false);
          (void)string_copy_bounded(valid, sizeof(valid), "Valid Data");
        } else {
          id = (MechId){0};
          (void)string_copy_bounded(valid, sizeof(valid),
                                    "Invalid Object Data!  Remove this Mech!");
        }
        notify_printf(btech_context_evaluation(map->xcode.context), player,
                      "Mech DB Number: %ld : [%s]\t%s", UNIT_DBREF, id.text,
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
}
