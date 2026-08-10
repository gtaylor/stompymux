/*
 * command.c - command parser and support routines
 */

#include "mux/communication/access_policy.h"
#include "mux/server/server_config.h" // IWYU pragma: keep

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"

bool is_in_character_location(GameDatabase *database,
                              const ServerConfiguration *configuration,
                              DbRef player) {
  DbRef d = game_object_location(database, player);
  int z = 0;

  while (is_player(database, d)) {
    DbRef od = d;

    d = game_object_location(database, d);
    if (d == od)
      break;
    if (z++ >= 100)
      break;
  }
  if (configuration->btech_ooc_comsys && !is_gagged(database, player))
    return 0;
  else if (is_in_character(database, d) || is_gagged(database, player))
    return 1;
  return 0;
} /* end In_IC_Loc() */
