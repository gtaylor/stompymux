/*
 * command.c - command parser and support routines
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/commands/macro.h"
#include "mux/communication/access_policy.h"
#include "mux/communication/comsys.h"
#include "mux/help/help_command.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"

#ifdef ARBITRARY_LOGFILES
#include "mux/server/log_cache.h"
#endif

bool is_in_character_location(GameDatabase *database,
                              const ServerConfiguration *configuration,
                              DbRef player) {
  DbRef d = game_object_location(database, player);
  int z = 0;

  while (is_player(database, d)) {
    DbRef od = d;

    if ((d = game_object_location(database, d)) == od)
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
