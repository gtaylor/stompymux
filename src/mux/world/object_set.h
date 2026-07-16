/* object_set.h - Object property, attribute, and lock mutation declarations. */

#pragma once

#include "mux/database/db.h"

DbRef match_controlled(DbRef player, char *name);
DbRef match_controlled_quiet(DbRef player, char *name);

void object_attribute_set(DbRef player, DbRef thing, int attribute_number,
                          char *attribute_text, int key);
int parse_attrib(DbRef player, char *string, DbRef *thing, int *attribute);
int parse_attrib_wild(DbRef player, char *string, DbRef *thing,
                      int check_parents, int get_locks, int default_star);
void edit_string(char *source, char **destination, const char *from,
                 const char *to);
