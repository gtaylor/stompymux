/** @file
 * Object property, attribute, and lock mutation declarations.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/world/walkdb.h"

typedef struct ObjectList ObjectList;
typedef struct MatchContext MatchContext;
typedef struct WorldIndexes WorldIndexes;

/** Executes match controlled. @param[in] match Match. @param[in] player Player
 * object. @param[in] name Name to use. */

DbRef match_controlled(MatchContext *match, DbRef player, const char *name);
/** Executes match controlled quiet. @param[in] match Match. @param[in] player
 * Player object. @param[in] name Name to use. */

DbRef match_controlled_quiet(MatchContext *match, DbRef player,
                             const char *name);

/** Executes edit string. @param[in,out] source Source value. @param[in,out]
 * destination Destination storage. @param[in] from From. @param[in] to To. */

void edit_string(char *source, char **destination, const char *from,
                 const char *to);
