/* Implements BattleTech commands for unit restrict. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_resume_api.h"
#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "btech_channel.h"
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

bool mech_map_position_is_valid(const MechMapPositionRequest *request) {
  const BattleMap *const BATTLE_MAP =
      btech_context_get_map(request->context, request->map);
  return (bool)(BATTLE_MAP != nullptr && request->x >= 0 && request->y >= 0 &&
                request->x < battle_map_width(BATTLE_MAP) &&
                request->y < battle_map_height(BATTLE_MAP));
}

bool mech_position_set(const MechPositionSetRequest *request) {
  Mech *mech = request->mech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  float fx;
  float fy;
  int elevation;

  if (!mech_map_position_is_valid(
          &(MechMapPositionRequest){.context = mech_context(mech),
                                    .map = mech_map_dbref(mech),
                                    .x = request->x,
                                    .y = request->y}))
    return false;

  mech_position_xy_set(mech, request->x, request->y);
  map_coord_to_real_coord(request->x, request->y, &fx, &fy);
  mech_position_real_xy_set(mech, (MapRealPosition){.x = fx, .y = fy});
  mark_for_los_update(mech);
  if (!request->has_z) {
    elevation =
        (unsigned char)map_elevation_get(mech_map, request->x, request->y);
    mech_position_z_set(mech, elevation - 1);
    mech_drop_surface_set(mech, false);
    mech_position_land_if_flying(mech);
  } else {
    mech_position_z_set(mech, request->z);
  }
  clear_mech_from_los(mech);
  return true;
}

void mech_rsetxy(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[3];
  int x;
  int y;
  int z = 0;
  int argc;

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
  if (argc == 3 && !parse_int_checked(args[2], &z)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid Z coordinate!");
    return;
  }
  if (!mech_position_set(&(MechPositionSetRequest){
          .mech = mech, .x = x, .y = y, .z = z, .has_z = argc == 3})) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid coordinates!");
    return;
  }
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Pos changed to %d,%d,%d", x, y, mech_position_z(mech));
}

static bool battle_map_has_room_for(const BattleMap *map, const Mech *mech) {
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    const DbRef UNIT = battle_map_unit_dbref(map, index);
    if (UNIT < 0 || UNIT == mech_dbref(mech))
      return true;
  }
  return battle_map_unit_count(map) < BATTLE_MAP_UNIT_CAPACITY;
}

static bool battle_map_unit_id_in_use(const BattleMap *map, const Mech *mech,
                                      const MechUnitId ID) {
  for (int index = 0; index < battle_map_unit_count(map); index++) {
    const DbRef UNIT = battle_map_unit_dbref(map, index);
    if (UNIT == mech_dbref(mech))
      continue;
    Mech *candidate = btech_context_get_mech(battle_map_context(map), UNIT);
    if (candidate != nullptr) {
      const MechUnitId CANDIDATE_ID = mech_unit_id(candidate);
      if (CANDIDATE_ID.first == ID.first && CANDIDATE_ID.second == ID.second)
        return true;
    }
  }
  return false;
}

static MechUnitId mech_map_unit_id_select(Mech *mech, const BattleMap *map,
                                          const char *preferred_id) {
  char *attribute;
  MechUnitId id;

  if (preferred_id != nullptr && strlen(preferred_id) > 1) {
    id = (MechUnitId){.first = *checked_string_suffix(preferred_id, 0),
                      .second = *checked_string_suffix(preferred_id, 1)};
  } else {
    attribute =
        btech_attribute_read(mech_context(mech)->database, mech_dbref(mech),
                             A_MECHPREFID, (char[LBUF_SIZE]){0});
    if (attribute != nullptr && strlen(attribute) > 1) {
      id = (MechUnitId){.first = *checked_string_suffix(attribute, 0),
                        .second = *checked_string_suffix(attribute, 1)};
    } else {
      id = (MechUnitId){.first = random_mech_id_character(mech_context(mech)),
                        .second = random_mech_id_character(mech_context(mech))};
    }
  }
  id.first = normalized_mech_id_character(id.first);
  id.second = normalized_mech_id_character(id.second);
  while (battle_map_unit_id_in_use(map, mech, id)) {
    id = (MechUnitId){.first = random_mech_id_character(mech_context(mech)),
                      .second = random_mech_id_character(mech_context(mech))};
  }
  return id;
}

MechMapSetResult mech_map_index_set(Mech *mech, DbRef map,
                                    const char *preferred_id) {
  BattleMap *destination = nullptr;
  BattleMap *current = nullptr;
  MechUnitId id = {0};

  if (map < -1)
    return MECH_MAP_SET_INVALID_DESTINATION;
  if (map != -1) {
    destination = btech_context_get_map(mech_context(mech), map);
    if (destination == nullptr)
      return MECH_MAP_SET_INVALID_DESTINATION;
    if (!battle_map_has_room_for(destination, mech))
      return MECH_MAP_SET_FULL;
    id = mech_map_unit_id_select(mech, destination, preferred_id);
  }
  if (mech_map_dbref(mech) != -1) {
    current = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    if (current == nullptr)
      return MECH_MAP_SET_INVALID_CURRENT_MAP;
  }

  if (current != nullptr) {
    mech_targeting_tag_clear(mech);
    mech_c3i_network_clear(mech, 1);
    mech_c3_network_clear(mech, true);
    remove_mech_from_map(current, mech);
  }
  if (destination == nullptr)
    return MECH_MAP_SET_OK;

  add_mech_to_map(destination, mech);
  mech_unit_id_set(mech, id.first, id.second);
  if (mech_position_x(mech) >= destination->map_width ||
      mech_position_y(mech) >= destination->map_height) {
    float fx;
    float fy;
    mech_position_reset_origin(mech);
    map_coord_to_real_coord(0, 0, &fx, &fy);
    mech_position_real_xy_set(mech, (MapRealPosition){.x = fx, .y = fy});
  }
  return MECH_MAP_SET_OK;
}

static Mech *mech_map_batch_unit_at(const MechMapSetBatchRequest *request,
                                    size_t index) {
  return *(Mech *const *)checked_storage_at_const(
      (const void *)request->mechs, request->count, sizeof(*request->mechs),
      index);
}

static bool battle_map_contains_unit(const BattleMap *map, DbRef unit) {
  for (int index = 0; index < battle_map_unit_count(map); index++)
    if (battle_map_unit_dbref(map, index) == unit)
      return true;
  return false;
}

MechMapSetResult
mech_map_index_preflight_batch(const MechMapSetBatchRequest *request) {
  BattleMap *destination = nullptr;
  size_t occupied = 0;
  size_t incoming = 0;

  if (request == nullptr || request->mechs == nullptr || request->count == 0 ||
      request->map < -1)
    return MECH_MAP_SET_INVALID_DESTINATION;
  Mech *const FIRST = mech_map_batch_unit_at(request, 0);
  if (FIRST == nullptr)
    return MECH_MAP_SET_INVALID_CURRENT_MAP;
  if (request->map != -1) {
    destination = btech_context_get_map(mech_context(FIRST), request->map);
    if (destination == nullptr)
      return MECH_MAP_SET_INVALID_DESTINATION;
    for (int index = 0; index < battle_map_unit_count(destination); index++)
      if (battle_map_unit_dbref(destination, index) >= 0)
        occupied++;
  }

  for (size_t index = 0; index < request->count; index++) {
    Mech *const MECH = mech_map_batch_unit_at(request, index);
    if (MECH == nullptr || mech_context(MECH) != mech_context(FIRST))
      return MECH_MAP_SET_INVALID_CURRENT_MAP;
    for (size_t prior = 0; prior < index; prior++)
      if (mech_map_batch_unit_at(request, prior) == MECH)
        return MECH_MAP_SET_INVALID_CURRENT_MAP;
    if (mech_map_dbref(MECH) != -1 &&
        btech_context_get_map(mech_context(MECH), mech_map_dbref(MECH)) ==
            nullptr)
      return MECH_MAP_SET_INVALID_CURRENT_MAP;
    if (destination != nullptr &&
        !battle_map_contains_unit(destination, mech_dbref(MECH)))
      incoming++;
  }
  if (destination != nullptr && occupied + incoming > BATTLE_MAP_UNIT_CAPACITY)
    return MECH_MAP_SET_FULL;
  return MECH_MAP_SET_OK;
}

MechMapSetResult
mech_map_index_set_batch(const MechMapSetBatchRequest *request) {
  MechMapSetResult result = mech_map_index_preflight_batch(request);
  if (result != MECH_MAP_SET_OK)
    return result;
  for (size_t index = 0; index < request->count; index++) {
    result = mech_map_index_set(mech_map_batch_unit_at(request, index),
                                request->map, nullptr);
    if (result != MECH_MAP_SET_OK) {
      btech_channel_send(mech_context(mech_map_batch_unit_at(request, index)),
                         BTECH_CHANNEL_MECH_ERRORS,
                         "Map placement batch failed after preflight at unit "
                         "#%ld (result %d).",
                         mech_dbref(mech_map_batch_unit_at(request, index)),
                         (int)result);
      return result;
    }
  }
  return MECH_MAP_SET_OK;
}

/* Team/Map commands */
void mech_rsetmapindex(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2];
  DbRef map;
  const int ARGUMENT_COUNT = mech_parseattributes(buffer, args, 2);

  if (ARGUMENT_COUNT < 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to SETMAPINDX!");
    return;
  }
  if (!parse_long_checked(args[0], &map) || map < -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid map index!");
    return;
  }

  const int OLD_X = mech_position_x(mech);
  const int OLD_Y = mech_position_y(mech);
  const MechMapSetResult RESULT =
      mech_map_index_set(mech, map, ARGUMENT_COUNT > 1 ? args[1] : nullptr);
  if (RESULT == MECH_MAP_SET_INVALID_DESTINATION) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid map index!");
    return;
  }
  if (RESULT == MECH_MAP_SET_INVALID_CURRENT_MAP) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Current map index is invalid!");
    return;
  }
  if (RESULT == MECH_MAP_SET_FULL) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are too many mechs on that map!");
    return;
  }
  if (map == -1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Mech removed from map.");
    return;
  }
  if ((OLD_X != 0 || OLD_Y != 0) && mech_position_x(mech) == 0 &&
      mech_position_y(mech) == 0) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You're current position is out of bounds, Pos changed to 0,0");
  }
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "MapIndex changed to %ld", map);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Your ID: %c%c", mech_unit_id(mech).first,
                mech_unit_id(mech).second);
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
    mech_template_clear(new, true);
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
