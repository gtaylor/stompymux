
/*
   p.mech.tech.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:58 CET 1999 from mech.tech.c */

#pragma once

typedef struct BtechContext BtechContext;

/* mech.tech.c */
int game_lag(BtechContext *context);
int game_lag_time(BtechContext *context, int duration);
int player_techtime(BtechContext *context, DbRef player);
int tech_roll(DbRef player, MECH *mech, int diff);
int tech_weapon_roll(DbRef player, MECH *mech, int diff);
void tech_status(BtechContext *context, DbRef player, time_t dat);
int tech_addtechtime(BtechContext *context, DbRef player, int time);
int tech_parsepart_advanced(MECH *mech, char *buffer, int *loc, int *pos,
                            int *extra, int allowrear);
int tech_parsepart(MECH *mech, char *buffer, int *loc, int *pos, int *extra);
int tech_parsegun(MECH *mech, char *buffer, int *loc, int *pos, int *brand);
int figure_latest_tech_event(MECH *mech);
