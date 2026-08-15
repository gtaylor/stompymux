/* Implements BattleTech commands for unit restrict. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_resume_api.h"
#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_dynamic_api.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_build_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_template_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"

static char random_mech_id_character(BtechContext *context) {
  const int OFFSET = btech_random_range_int(context, 0, 25);
  return (char)('A' + OFFSET);
}

static char normalized_mech_id_character(char value) {
  const int UPPERCASE = (unsigned char)ascii_to_upper(value);
  return (char)bounded('A', UPPERCASE, 'Z');
}

void clear_mech_from_los(Mech *mech) {
  BattleMap *map;
  int i;
  Mech *mek;

  /* if (mech_map_dbref(mech) < 0)
     return;
   */
  map = btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  if (!map)
    return;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    battle_map_los_flags_set(map, mech_map_slot(mech), i, 0);
    battle_map_los_flags_set(map, i, mech_map_slot(mech), 0);

    const DbRef UNIT = battle_map_unit_dbref(map, i);
    if (UNIT >= 0 && i != mech_map_slot(mech)) {
      mek = btech_context_get_mech(mech_context(mech), UNIT);
      if (!mek)
        continue;
      if (mech_targeting_has_lock_on(mek, mech_dbref(mech))) {
        mech_notify(mek, MECHALL,
                    "Weapon system reports the lock has been lost.");
        mech_lose_lock(mek);
      }
      if ((battle_map_los_flags(map, i, mech_map_slot(mech)) &
           BATTLE_MAP_LOS_SEEN) &&
          mech_team(mek) != mech_team(mech))
        mech_seen_count_decrement(mek);
    }
  }
  if (mech_targeting_lock_modes_active(mech)) {
    mech_notify(mech, MECHALL, "Weapon system reports the lock has been lost.");
    mech_lose_lock(mech);
  }
}

void mech_rsetxy(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *args[3];
  int x;
  int y;
  int z;
  int argc;
  float fx;
  float fy;
  int elevation;

  if (!common_checks(player, mech, MECH_MAP))
    return;
  argc = mech_parseattributes(buffer, args, 3);
  if (argc != 2 && argc != 3) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to SETXY!");
    return;
  }
  if (!parse_int_checked(args[0], &x) || !parse_int_checked(args[1], &y)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid coordinates!");
    return;
  }
  if (x >= mech_map->map_width || y >= mech_map->map_height || x < 0 || y < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid coordinates!");
    return;
  }
  mech_position_xy_set(mech, x, y);
  map_coord_to_real_coord(x, y, &fx, &fy);
  mech_position_real_xy_set(mech, (MapRealPosition){.x = fx, .y = fy});
  mark_for_los_update(mech);
  if (argc == 2) {
    elevation = (unsigned char)map_elevation_get(mech_map, x, y);
    mech_position_z_set(mech, elevation - 1);
    mech_drop_surface_set(mech, false);
    z = mech_position_z(mech);
    mech_position_land_if_flying(mech);
  } else {
    if (!parse_int_checked(args[2], &z)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid Z coordinate!");
      return;
    }
    mech_position_z_set(mech, z);
  }
  clear_mech_from_los(mech);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Pos changed to %d,%d,%d", x, y, z);
}

/* Team/Map commands */
void mech_rsetmapindex(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2];
  char *tempstr;
  int newindex;
  int nargs;
  int notdone = 0;
  int loop;
  BattleMap *newmap = nullptr;
  BattleMap *oldmap;
  Mech *temp_mech;
  char targ[2];

  nargs = mech_parseattributes(buffer, args, 2);
  if (nargs < 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to SETMAPINDX!");
    return;
  }
  if (!parse_int_checked(args[0], &newindex) || newindex < -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid map index!");
    return;
  }
  if (newindex != -1) {
    newmap = valid_map(&(MapValidationRequest){
        .context = mech_context(mech), .player = player, .map = newindex});
    if (!newmap)
      return;
  }
  /* Remove the mech from it's old map */
  if (mech_map_dbref(mech) != -1) {
    oldmap = valid_map(&(MapValidationRequest){.context = mech_context(mech),
                                               .player = player,
                                               .map = mech_map_dbref(mech)});
    if (!oldmap)
      return;
    mech_targeting_tag_clear(mech);
    mech_c3i_network_clear(mech, 1);
    mech_c3_network_clear(mech, 1);
    remove_mech_from_map(oldmap, mech);
  }

  if (newindex == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Mech removed from map.");
    return;
  }

  /* Just make it random */
  /* Find a clear spot for this mech */
  const char *preferred_id =
      nargs > 1 ? *(char **)checked_storage_at((void *)args, (size_t)nargs,
                                               sizeof(*args), 1)
                : nullptr;
  if (preferred_id != nullptr && strlen(preferred_id) > 1) {
    targ[0] = *checked_string_suffix(preferred_id, 0);
    targ[1] = *checked_string_suffix(preferred_id, 1);
  } else {
    tempstr =
        btech_attribute_read(mech_context(mech)->database, mech_dbref(mech),
                             A_MECHPREFID, (char[LBUF_SIZE]){0});
    if (tempstr && strlen(tempstr) > 1) {
      targ[0] = *checked_string_suffix(tempstr, 0);
      targ[1] = *checked_string_suffix(tempstr, 1);
    } else {
      targ[0] = random_mech_id_character(mech_context(mech));
      targ[1] = random_mech_id_character(mech_context(mech));
    }
  }
  targ[0] = normalized_mech_id_character(targ[0]);
  targ[1] = normalized_mech_id_character(targ[1]);
  for (loop = 0; (loop < battle_map_unit_count(newmap) && !notdone); loop++) {
    const DbRef UNIT = battle_map_unit_dbref(newmap, loop);
    temp_mech = (Mech *)btech_context_find_object(mech_context(mech), UNIT);
    if (temp_mech) {
      MechUnitId const ID = mech_unit_id(temp_mech);
      if (ID.first == targ[0] && ID.second == targ[1])
        notdone = 1;
    }
  }
  while (notdone) {
    targ[0] = random_mech_id_character(mech_context(mech));
    targ[1] = random_mech_id_character(mech_context(mech));
    notdone = 0;
    for (loop = 0; (loop < battle_map_unit_count(newmap) && !notdone); loop++) {
      const DbRef UNIT = battle_map_unit_dbref(newmap, loop);
      temp_mech = (Mech *)btech_context_find_object(mech_context(mech), UNIT);
      if (temp_mech) {
        MechUnitId const ID = mech_unit_id(temp_mech);
        if (ID.first == targ[0] && ID.second == targ[1])
          notdone = 1;
      }
    }
  }
  if (loop == MAX_MECHS_PER_MAP) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are too many mechs on that map!");
    return;
  }
  add_mech_to_map(newmap, mech);
  mech_unit_id_set(mech, targ[0], targ[1]);
  if (mech_position_x(mech) > (newmap->map_width - 1) ||
      mech_position_y(mech) > (newmap->map_height - 1)) {
    float fx;
    float fy;
    mech_position_reset_origin(mech);
    map_coord_to_real_coord(0, 0, &fx, &fy);
    mech_position_real_xy_set(mech, (MapRealPosition){.x = fx, .y = fy});
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You're current position is out of bounds, Pos changed to 0,0");
  }
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "MapIndex changed to %d", newindex);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Your ID: %c%c", mech_unit_id(mech).first,
                mech_unit_id(mech).second);
  autopilot_resume_for_mech(mech);
}

void mech_rsetteam(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  int team;
  BattleMap *newmap;

  if (mech_map_dbref(mech) == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Mech is not on a map:  Can't set team");
    return;
  }
  newmap = valid_map(&(MapValidationRequest){.context = mech_context(mech),
                                             .player = player,
                                             .map = mech_map_dbref(mech)});
  if (!newmap) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Map index reset!");
    mech_map_dbref_set(mech, NOTHING);
    return;
  }
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }
  if (!parse_int_checked(args[0], &team)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid team!");
    return;
  }
  if (team < 0)
    team = 0;
  mech_team_set(mech, team);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Team set to %d", team);
}

/* Alloc/free routine */
void newfreemech(DbRef key, void **data,
                 BtechSpecialLifecycleOperation selector) {
  Mech *new = *data;
  BattleMap *map;
  int i;
  AutopilotCommand *temp;

  switch (selector) {
  case SPECIAL_ALLOC:
    mech_identity_initialize(new, key);
    mech_template_clear(new, 1);
    for (i = 0; i < NUM_SECTIONS; i++)
      fill_default_criticals(new, i);
    break;
  case SPECIAL_FREE:
    mech_stagger_damage_clear(new);
    if (mech_map_dbref(new) != -1) {
      map = btech_context_get_map(mech_context(new), mech_map_dbref(new));
      if (map)
        remove_mech_from_map(map, new);
    }
    if (mech_autopilot_dbref(new) > 0) {
      Autopilot *autopilot = btech_context_find_object(
          mech_context(new), mech_autopilot_dbref(new));
      if (autopilot) {
        auto_stop_pilot(autopilot);
        /* Go through the list and remove any leftover nodes */
        while (doubly_linked_list_size(autopilot->commands)) {

          /* Remove the first node on the list and get the data
           * from it */
          temp = (AutopilotCommand *)doubly_linked_list_remove(
              autopilot->commands,
              doubly_linked_list_head(autopilot->commands));

          /* Destroy the command node */
          auto_destroy_command_node(temp);
        }

        /* Destroy the list */
        doubly_linked_list_destroy_list(autopilot->commands);
        autopilot->commands = nullptr;

        /* Destroy any astar path list thats on the AI */
        auto_destroy_astar_path(autopilot);

        /* Destroy profile storage. */
        autopilot_weapon_profiles_clear(autopilot);

        /* Destroy weaponlist */
        auto_destroy_weaplist(autopilot);

        autopilot->mymechnum = -1;
      }
      mech_autopilot_dbref_set(new, -1);
    }
  }
}
