/* Declares the BattleTech unit tech damages API. */

#pragma once

#include "mux/server/platform.h"

/* mech.tech.damages.c */
size_t mech_repair_job_count(Mech *mech);
void mech_repair_jobs_format(Mech *mech, char *buffer, size_t buffer_size);
void show_mechs_damage(DbRef player, void *data, char *buffer);
void tech_fix(DbRef player, void *data, char *buffer);
