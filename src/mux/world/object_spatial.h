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
int nearby(GameDatabase *database, DbRef player, DbRef object);
typedef struct ExitVisibilityRequest {
  EvaluationContext *evaluation;
  GameDatabase *database;
  DbRef exit;
  DbRef viewer;
  int options;
} ExitVisibilityRequest;

bool exit_visible(const ExitVisibilityRequest *request);
bool exit_displayable(const ExitVisibilityRequest *request);
