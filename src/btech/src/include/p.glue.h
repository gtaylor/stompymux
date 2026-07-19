
/*
   p.glue.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:36 CET 1999 from glue.c */

#pragma once

#include <stddef.h>

typedef struct BtechContext BtechContext;
typedef struct EvaluationContext EvaluationContext;

/* glue.c */
int HandledCommand_sub(BtechContext *context, DbRef player, DbRef location,
                       char *command);
int HandledCommand(BtechContext *context, DbRef player, DbRef loc,
                   char *command);
void mech_remove_from_all_maps(MECH *mech);
void mech_remove_from_all_maps_except(MECH *mech, int num);
void LoadSpecialObjects(BtechContext *context);
void UpdateSpecialObjects(BtechContext *context);
void *NewSpecialObject(BtechContext *context, long id, int type);
void CreateNewSpecialObject(BtechContext *context, DbRef player, DbRef key);
void DisposeSpecialObject(BtechContext *context, DbRef player, DbRef key);
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
void handle_xcode(BtechContext *context, DbRef player, DbRef obj, int from,
                  int to);
void initialize_colorize(BtechContext *context);
void destroy_colorize(BtechContext *context);
char *colorize(EvaluationContext *evaluation, DbRef player, char *from);
void mecha_notify(EvaluationContext *evaluation, DbRef player, char *msg);
void mecha_notify_except(EvaluationContext *evaluation, DbRef loc, DbRef player,
                         DbRef exception, char *msg);
void list_chashstats(DbRef player);
void ResetSpecialObjects(BtechContext *context);
MAP *btech_context_get_map(BtechContext *context, DbRef d);
MECH *btech_context_get_mech(BtechContext *context, DbRef d);
