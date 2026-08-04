/* Shared in-character communication access policy. */

#pragma once

#include "mux/objects/db.h"

typedef struct ServerConfiguration ServerConfiguration;

bool is_in_character_location(GameDatabase *database,
                              const ServerConfiguration *configuration,
                              DbRef player);
