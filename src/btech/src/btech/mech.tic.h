
/*
 * $Id: mech.tic.h,v 1.1.1.1 2005/01/11 21:18:26 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Fri Nov 22 20:42:02 1996 fingon
 * Last modified: Mon May 26 14:26:03 1997 fingon
 *
 */

#pragma once

#include "mech.h"

/* mech.tic.c */
void cleartic_sub(DbRef player, MECH *mech, char *buffer);
void addtic_sub(DbRef player, MECH *mech, char *buffer);
void deltic_sub(DbRef player, MECH *mech, char *buffer);
void firetic_sub(DbRef player, MECH *mech, char *buffer);
void listtic_sub(DbRef player, MECH *mech, char *buffer);
void mech_cleartic(DbRef player, void *data, char *buffer);
void mech_addtic(DbRef player, void *data, char *buffer);
void mech_deltic(DbRef player, void *data, char *buffer);
void mech_firetic(DbRef player, void *data, char *buffer);
void mech_listtic(DbRef player, void *data, char *buffer);
void heat_cutoff(DbRef player, void *data, char *buffer);
