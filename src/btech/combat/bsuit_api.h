
/*
   p.bsuit.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:11 CET 1999 from bsuit.c */

#pragma once

#include "map.h"
#include "mech.h"
#include "mux/server/platform.h"

/* bsuit.c */
char *GetBSuitName(Mech *mech);
char *GetLCaseBSuitName(Mech *mech);
void StartBSuitRecycle(Mech *mech, int time);
void StopSwarming(Mech *mech, int intentional);
int CountSwarmers(Mech *mech);
Mech *findSwarmers(Mech *mech);
void StopBSuitSwarmers(BattleMap *map, Mech *mech, int intentional);
int IsMechSwarmed(Mech *mech);
int IsMechMounted(Mech *mech);
void BSuitMirrorSwarmedTarget(BattleMap *map, Mech *mech);
int doBSuitCommonChecks(Mech *mech, DbRef player);
int CountBSuitMembers(Mech *mech);
int FindBSuitTarget(DbRef player, Mech *mech, Mech **target, char *buffer);
int doJettisonChecks(Mech *mech);
void bsuit_swarm(DbRef player, void *data, char *buffer);
void bsuit_attackleg(DbRef player, void *data, char *buffer);
void bsuit_hide(DbRef player, void *data, char *buffer);
void JettisonPacks(DbRef player, void *data, char *buffer);
