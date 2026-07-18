/* object_spatial.h - Object containment, location, and exit visibility
 * interface. */

#pragma once

#include "mux/database/db.h"

typedef struct ServerConfiguration ServerConfiguration;
typedef struct EvaluationContext EvaluationContext;

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
