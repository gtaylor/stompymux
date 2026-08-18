/** @file
 * Shared in-character communication access policy.
 */
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

// IWYU pragma: no_include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

/** Reports whether is in character location. @param[in] database Game database.
 * @param[in] configuration Server configuration. @param[in] player Player
 * object. */

bool is_in_character_location(GameDatabase *database,
                              const ServerConfiguration *configuration,
                              DbRef player);
