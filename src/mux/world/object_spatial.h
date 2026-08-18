/** @file
 * Object containment, location, and exit visibility interface.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;
typedef struct EvaluationContext EvaluationContext;

constexpr int VE_LOC_XAM = 0x01;  /* Location is examinable. */
constexpr int VE_LOC_DARK = 0x02; /* Location is dark. */

/** Executes where is. @param[in] database Game database. @param[in] what What.
 */

DbRef where_is(GameDatabase *database, DbRef what);
/** Executes where room. @param[in,out] database Game database. @param[in]
 * configuration Server configuration. @param[in] what What. */

DbRef where_room(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef what);
/** Executes nearby. @param[in,out] database Game database. @param[in] player
 * Player object. @param[in] thing Thing. */

bool nearby(GameDatabase *database, DbRef player, DbRef thing);
typedef struct ExitVisibilityRequest {
  EvaluationContext *evaluation;
  GameDatabase *database;
  DbRef exit;
  DbRef viewer;
  int options;
} ExitVisibilityRequest;

/** Executes exit visible. @param[in] request Request. */

bool exit_visible(const ExitVisibilityRequest *request);
/** Executes exit displayable. @param[in] request Request. */

bool exit_displayable(const ExitVisibilityRequest *request);
