/* object_set.h - Object property, attribute, and lock mutation declarations. */

#pragma once

#include "mux/database/db.h"

typedef struct ObjectList ObjectList;
typedef struct MatchContext MatchContext;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct WorldIndexes WorldIndexes;

DbRef match_controlled(MatchContext *match, DbRef player, char *name);
DbRef match_controlled_quiet(MatchContext *match, DbRef player, char *name);

void object_attribute_set(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player, DbRef thing, int attribute_number,
                          char *attribute_text, int key);
int parse_attrib(MatchContext *match, DbRef player, char *string, DbRef *thing,
                 int *attribute);
void edit_string(char *source, char **destination, const char *from,
                 const char *to);
