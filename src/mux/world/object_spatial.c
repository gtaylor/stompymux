/* object_spatial.c - Object containment, location, and exit visibility checks.
 */

#include "mux/world/object_spatial.h"

#include "mux/server/platform.h"

#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/world/world_context.h"

DbRef where_is(GameDatabase *database, DbRef what) {
  DbRef loc;

  if (!is_good_obj(database, what))
    return NOTHING;

  switch (typeof_obj(database, what)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    loc = game_object_location(database, what);
    break;
  case TYPE_EXIT:
    loc = game_object_exits(database, what);
    break;
  default:
    loc = NOTHING;
    break;
  }
  return loc;
}

/**
 * Return room containing player, or NOTHING if no room or
 * recursion exceeded.  If player is a room, returns itself.
 */
DbRef where_room(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef what) {
  int count;

  for (count = configuration->ntfy_nest_lim; count > 0; count--) {
    if (!is_good_obj(database, what))
      break;
    if (is_room(database, what))
      return what;
    if (!has_location(database, what))
      break;
    what = game_object_location(database, what);
  }
  return NOTHING;
}

int locatable(EvaluationContext *evaluation,
              const ServerConfiguration *configuration, DbRef player, DbRef it,
              DbRef cause) {
  (void)configuration;
  (void)player;
  (void)cause;
  return is_good_obj(evaluation->world->database, it);
}

/**
 * Check if thing is nearby player (in inventory, in same room, or
 * IS the room.
 */
int nearby(GameDatabase *database, DbRef player, DbRef thing) {
  DbRef thing_loc, player_loc;

  if (!is_good_obj(database, player) || !is_good_obj(database, thing))
    return 0;
  thing_loc = where_is(database, thing);
  if (thing_loc == player)
    return 1;
  player_loc = where_is(database, player);
  if ((thing_loc == player_loc) || (thing == player_loc))
    return 1;
  return 0;
}

/**
 * Checks to see if the exit is visible. Used in lexits().
 */
int exit_visible(EvaluationContext *evaluation, DbRef exit, DbRef player,
                 int key) {
  if (key & VE_LOC_XAM) // Exam exit's loc
    return 1;
  if (is_examinable(evaluation->world->database, player, exit)) // Exam exit
    return 1;
  if (is_light(evaluation->world->database, exit)) // Exit is light
    return 1;
  if (key & VE_LOC_DARK)
    return 0;                                     // Dark loc or base
  if (is_dark(evaluation->world->database, exit)) // Dark exit
    return 0;
  return 1; // Default
}

/**
 * Checks to see if the exit is visible to look.
 */
int exit_displayable(GameDatabase *database, DbRef exit, DbRef player,
                     int key) {
  if (is_dark(database, exit)) // Dark exit
    return 0;
  if (is_light(database, exit)) // Light exit
    return 1;
  if (key & VE_LOC_DARK)
    return 0; // Dark loc or base
  return 1;   // Default
}
