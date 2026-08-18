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

/** Executes object attribute is administrable. @param[in] attribute_number
 * Attribute number. */

bool object_attribute_is_administrable(int attribute_number);
/** Executes object attribute administrable by name. @param[in] database Game
 * database. @param[in] name Name to use. */

const Attribute *object_attribute_administrable_by_name(GameDatabase *database,
                                                        const char *name);
/** Sets object attribute. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] thing Thing. @param[in]
 * attrnum Attrnum. @param[in,out] attrtext Attrtext. @param[in] key Lookup key
 * or command flags. */

bool object_attribute_set(EvaluationContext *evaluation, DbRef player,
                          DbRef thing, int attrnum, char *attrtext, int key);
/** Executes edit string. @param[in,out] source Source value. @param[in,out]
 * destination Destination storage. @param[in] from From. @param[in] to To. */

void edit_string(char *source, char **destination, const char *from,
                 const char *to);
