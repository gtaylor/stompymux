/* Declares the BattleTech artillery API. */

#pragma once

#include "mux/server/platform.h"

/* artillery.c */
int artillery_round_flight_time(float fx, float fy, float tx, float ty);
void artillery_shoot(Mech *mech, int targx, int targy, int windex, int wmode,
                     int ishit);
void blast_hit_hexf(BattleMap *map, int dam, int singlehitsize, int heatdam,
                    float fx, float fy, float tfx, float tfy, const char *tomsg,
                    const char *otmsg, int table, int safeup, int safedown,
                    int isunderwater);
void blast_hit_hex(BattleMap *map, int dam, int singlehitsize, int heatdam,
                   int fx, int fy, int tx, int ty, const char *tomsg,
                   const char *otmsg, int table, int safeup, int safedown,
                   int isunderwater);
void blast_hit_hexesf(BattleMap *map, int dam, int singlehitsize, int heatdam,
                      float fx, float fy, float ftx, float fty,
                      const char *tomsg, const char *otmsg, const char *tomsg1,
                      const char *otmsg1, int table, int safeup, int safedown,
                      int isunderwater, int doneighbors);
void blast_hit_hexes(BattleMap *map, int dam, int singlehitsize, int heatdam,
                     int tx, int ty, const char *tomsg, const char *otmsg,
                     const char *tomsg1, const char *otmsg1, int table,
                     int safeup, int safedown, int isunderwater,
                     int doneighbors);
void artillery_friendly_adjustment(DbRef mechnum, BattleMap *map, int x, int y);
