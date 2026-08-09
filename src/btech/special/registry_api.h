/* Declares the BattleTech registry API. */

#pragma once

#include "mux/server/platform.h"

#include <stddef.h>

typedef struct BtechContext BtechContext;
typedef struct EvaluationContext EvaluationContext;

/* registry.c */
int HandledCommand_sub(BtechContext *context, DbRef player, DbRef location,
                       char *command);
bool btech_command_try_execute(BtechContext *context, DbRef player, DbRef loc,
                               char *command);
void mech_remove_from_all_maps(Mech *mech);
void mech_remove_from_all_maps_except(Mech *mech, DbRef num);
void btech_special_objects_load(BtechContext *context);
void btech_special_objects_update(BtechContext *context);
void *NewSpecialObject(BtechContext *context, DbRef id, int type);
void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key);
void btech_special_object_dispose(BtechContext *context, DbRef player,
                                  DbRef key);
void Dump_Mech(BtechContext *context, DbRef player, int type, char *typestr);
void DumpMechs(BtechContext *context, DbRef player);
void DumpMaps(BtechContext *context, DbRef player);
int btech_context_which_special(BtechContext *context, DbRef key);
bool btech_context_is_mech(BtechContext *context, DbRef key);
bool btech_context_is_auto(BtechContext *context, DbRef key);
bool btech_context_is_map(BtechContext *context, DbRef key);
void *btech_context_find_object(BtechContext *context, DbRef key);
void center_string(char *destination, size_t destination_size,
                   const char *source, int width);
void InitSpecialHash(BtechContext *context, int which);
void btech_special_object_flag_changed(BtechContext *context, DbRef player,
                                       DbRef obj, bool from, bool to);
void mecha_notify(EvaluationContext *evaluation, DbRef player, const char *msg);
void mecha_notify_except(EvaluationContext *evaluation, DbRef loc, DbRef player,
                         DbRef exception, const char *msg);
void list_chashstats(DbRef player);
void btech_special_objects_reset(BtechContext *context);
int btech_special_object_type_count(void);
const char *btech_special_object_type_name(int type);
size_t btech_special_object_storage_size(int type);
BattleMap *btech_context_get_map(BtechContext *context, DbRef d);
Mech *btech_context_get_mech(BtechContext *context, DbRef d);
