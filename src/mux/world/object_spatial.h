/* object_spatial.h - Object containment, location, and exit visibility
 * interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;
typedef struct EvaluationContext EvaluationContext;

constexpr int VE_LOC_XAM = 0x01;  /* Location is examinable. */
constexpr int VE_LOC_DARK = 0x02; /* Location is dark. */

DbRef where_is(GameDatabase *database, DbRef object);
DbRef where_room(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef object);
int locatable(EvaluationContext *evaluation,
              const ServerConfiguration *configuration, DbRef player,
              DbRef object, DbRef cause);
int nearby(GameDatabase *database, DbRef player, DbRef object);
int exit_visible(EvaluationContext *evaluation, DbRef exit, DbRef player,
                 int key);
int exit_displayable(GameDatabase *database, DbRef exit, DbRef player, int key);
