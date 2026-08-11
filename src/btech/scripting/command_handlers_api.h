/* Declares the BattleTech command handlers API. */

#pragma once

#include "mux/server/platform.h"

#include "mux/support/alloc.h"

typedef struct GameDatabase GameDatabase;
typedef struct EvaluationContext EvaluationContext;

/* functions.c */
char *btech_attribute_read(GameDatabase *database, DbRef id, int flag,
                           char buffer[static LBUF_SIZE]);
void silly_atr_set_in(GameDatabase *database, DbRef id, int flag,
                      const char *data);
void kill_text(char **lines, size_t count);
void free_text_items(char **lines, size_t count);
void show_text(EvaluationContext *evaluation, char **lines, size_t count,
               DbRef player);
float fbounded(float min, float val, float max);
int bounded(int min, int val, int max);
int max(int v1, int v2);
int min(int v1, int v2);
int silly_parseattributes(char *buffer, char **args, int max);
int mech_parseattributes(char *buffer, char **args, int maxargs);
int proper_parseattributes(char *buffer, char **args, int max);
int proper_explodearguments(const char *buffer, char **args, int max);
char *first_parseattribute(char *buffer);
