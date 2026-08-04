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

void object_attribute_set(EvaluationContext *evaluation, DbRef player,
                          DbRef thing, int attribute_number,
                          char *attribute_text, int key);
void edit_string(char *source, char **destination, const char *from,
                 const char *to);
