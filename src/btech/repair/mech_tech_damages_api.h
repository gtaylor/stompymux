/* Declares the BattleTech unit tech damages API. */

#pragma once

#include "mux/server/platform.h"

#include <stdbool.h>

typedef struct BtechRepairNeed {
  int operation;
  int section;
  int detail;
  bool in_progress;
} BtechRepairNeed;

typedef bool (*BtechRepairNeedVisitor)(const BtechRepairNeed *need,
                                       void *context);

/** Visits an immutable snapshot of a unit's required repair operations. */
void btech_repair_needs_visit(Mech *mech, BtechRepairNeedVisitor visitor,
                              void *context);

/* mech.tech.damages.c */
size_t mech_repair_job_count(Mech *mech);
void mech_repair_jobs_format(Mech *mech, char *buffer, size_t buffer_size);
void show_mechs_damage(DbRef player, Mech *mech, char *buffer);
void tech_fix(DbRef player, Mech *mech, char *buffer);
