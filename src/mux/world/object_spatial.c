/* object_spatial.c - Object containment, location, and exit visibility checks. */

#include "mux/world/object_spatial.h"

#include "mux/server/platform.h"

#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"

DbRef where_is(DbRef what) {
  DbRef loc;

  if (!Good_obj(what))
    return NOTHING;

  switch (Typeof(what)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    loc = Location(what);
    break;
  case TYPE_EXIT:
    loc = Exits(what);
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
DbRef where_room(DbRef what) {
  int count;

  for (count = mudconf.ntfy_nest_lim; count > 0; count--) {
    if (!Good_obj(what))
      break;
    if (isRoom(what))
      return what;
    if (!Has_location(what))
      break;
    what = Location(what);
  }
  return NOTHING;
}

int locatable(DbRef player, DbRef it, DbRef cause) {
  DbRef loc_it, room_it;
  int findable_room;

  /*
   * No sense if trying to locate a bad object
   */

  if (!Good_obj(it))
    return 0;

  loc_it = where_is(it);

  /*
   * Succeed if we can examine the target, if we are the target, * if *
   *
   * *  * * we can examine the location, if a wizard caused the lookup,
   * * or  * *  * if the target caused the lookup.
   */

  if (Examinable(player, it) || Find_Unfindable(player) || (loc_it == player) ||
      ((loc_it != NOTHING) &&
       (Examinable(player, loc_it) || loc_it == where_is(player))) ||
      Wizard(cause) || (it == cause))
    return 1;

  room_it = where_room(it);
  if (Good_obj(room_it))
    findable_room = !Hideout(room_it);
  else
    findable_room = 1;

  /*
   * Succeed if we control the containing room or if the target is * *
   * * * findable and the containing room is not unfindable.
   */

  if (((room_it != NOTHING) && Examinable(player, room_it)) ||
      Find_Unfindable(player) || (Findable(it) && findable_room))
    return 1;

  /*
   * We can't do it.
   */

  return 0;
}

/**
 * Check if thing is nearby player (in inventory, in same room, or
 * IS the room.
 */
int nearby(DbRef player, DbRef thing) {
  int thing_loc, player_loc;

  if (!Good_obj(player) || !Good_obj(thing))
    return 0;
  thing_loc = where_is(thing);
  if (thing_loc == player)
    return 1;
  player_loc = where_is(player);
  if ((thing_loc == player_loc) || (thing == player_loc))
    return 1;
  return 0;
}

/**
 * Checks to see if the exit is visible. Used in lexits().
 */
int exit_visible(DbRef exit, DbRef player, int key) {
  if (key & VE_LOC_XAM) // Exam exit's loc
    return 1;
  if (Examinable(player, exit)) // Exam exit
    return 1;
  if (Light(exit)) // Exit is light
    return 1;
  if (key & (VE_LOC_DARK | VE_BASE_DARK))
    return 0;     // Dark loc or base
  if (Dark(exit)) // Dark exit
    return 0;
  return 1; // Default
}

/**
 * Checks to see if the exit is visible to look.
 */
int exit_displayable(DbRef exit, DbRef player, int key) {
  if (Dark(exit)) // Dark exit
    return 0;
  if (Light(exit)) // Light exit
    return 1;
  if (key & (VE_LOC_DARK | VE_BASE_DARK))
    return 0; // Dark loc or base
  return 1;   // Default
}
