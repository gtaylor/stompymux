
/*
   p.mech.scan.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Tue Feb  9 14:31:35 CET 1999 from mech.scan.c */

#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

/* mech.scan.c */
void mech_scan(DbRef player, void *data, char *buffer);
void mech_report(DbRef player, void *data, char *buffer);
void mech_scan_show_turret_facing(EvaluationContext *evaluation, DbRef player,
                                  int spaces, Mech *mech);
void mech_scan_print_report(EvaluationContext *evaluation, DbRef player,
                            Mech *mech, Mech *target, float range);
void mech_scan_print_enemy_status(EvaluationContext *evaluation, DbRef player,
                                  Mech *observer, Mech *target, float range,
                                  int options);
void mech_bearing(DbRef player, void *data, char *buffer);
void mech_range(DbRef player, void *data, char *buffer);
void mech_vector(DbRef player, void *data, char *buffer);
void PrintEnemyWeaponStatus(Mech *mech, DbRef player);
void mech_sight(DbRef player, void *data, char *buffer);
void mech_view(DbRef player, void *data, char *buffer);
