/* object_set.h - Object property, attribute, and lock mutation declarations. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/world/walkdb.h"

typedef struct ObjectList ObjectList;
typedef struct MatchContext MatchContext;
typedef struct WorldIndexes WorldIndexes;

DbRef match_controlled(MatchContext *match, DbRef player, char *name);
DbRef match_controlled_quiet(MatchContext *match, DbRef player, char *name);

bool object_attribute_is_administrable(int attribute_number);
const Attribute *object_attribute_administrable_by_name(GameDatabase *database,
                                                        const char *name);
bool object_attribute_set(EvaluationContext *evaluation, DbRef player,
                          DbRef thing, int attrnum, char *attrtext, int key);
void edit_string(char *source, char **destination, const char *from,
                 const char *to);
