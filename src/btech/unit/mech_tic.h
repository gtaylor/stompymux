
/* Declares unit tick-processing interfaces. */

#pragma once

#include "mech_internal.h"

/* mech.tic.c */
void cleartic_sub(DbRef player, Mech *mech, char *buffer);
void addtic_sub(DbRef player, Mech *mech, char *buffer);
void deltic_sub(DbRef player, Mech *mech, char *buffer);
void firetic_sub(DbRef player, Mech *mech, char *buffer);
void listtic_sub(DbRef player, Mech *mech, char *buffer);
void mech_cleartic(DbRef player, void *data, char *buffer);
void mech_addtic(DbRef player, void *data, char *buffer);
void mech_deltic(DbRef player, void *data, char *buffer);
void mech_firetic(DbRef player, void *data, char *buffer);
void mech_listtic(DbRef player, void *data, char *buffer);
void heat_cutoff(DbRef player, void *data, char *buffer);
