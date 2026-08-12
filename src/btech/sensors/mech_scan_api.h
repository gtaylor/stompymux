/* Declares the BattleTech unit scan API. */

#pragma once

#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

/* mech.scan.c */
void mech_scan(DbRef player, void *data, char *buffer);
void mech_report(DbRef player, void *data, char *buffer);
void mech_scan_show_turret_facing(EvaluationContext *evaluation, DbRef player,
                                  Mech *mech);
void mech_scan_print_report(EvaluationContext *evaluation, DbRef player,
                            Mech *mech, Mech *temp_mech, float range);
typedef struct ScanEnemyStatusRequest {
  EvaluationContext *evaluation;
  DbRef player;
  Mech *observer;
  Mech *target;
  float range;
  int options;
} ScanEnemyStatusRequest;
void mech_scan_print_enemy_status(const ScanEnemyStatusRequest *request);
void mech_bearing(DbRef player, void *data, char *buffer);
void mech_range(DbRef player, void *data, char *buffer);
void mech_vector(DbRef player, void *data, char *buffer);
void print_enemy_weapon_status(Mech *mech, DbRef player);
void mech_sight(DbRef player, void *data, char *buffer);
void mech_view(DbRef player, void *data, char *buffer);
