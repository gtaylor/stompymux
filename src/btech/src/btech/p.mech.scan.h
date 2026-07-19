
/*
   p.mech.scan.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Tue Feb  9 14:31:35 CET 1999 from mech.scan.c */

#pragma once

typedef struct EvaluationContext EvaluationContext;

/* mech.scan.c */
void mech_scan(DbRef player, void *data, char *buffer);
void mech_report(DbRef player, void *data, char *buffer);
void ShowTurretFacing(EvaluationContext *evaluation, DbRef player, int spaces,
                      MECH *mech);
void PrintReport(EvaluationContext *evaluation, DbRef player, MECH *mech,
                 MECH *tempMech, float range);
void PrintEnemyStatus(EvaluationContext *evaluation, DbRef player, MECH *mymech,
                      MECH *mech, float range, int opt);
void mech_bearing(DbRef player, void *data, char *buffer);
void mech_range(DbRef player, void *data, char *buffer);
void PrintEnemyWeaponStatus(MECH *mech, DbRef player);
void mech_sight(DbRef player, void *data, char *buffer);
void mech_view(DbRef player, void *data, char *buffer);
