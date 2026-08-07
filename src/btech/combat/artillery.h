

/*
 * $Id: artillery.h,v 1.1.1.1 2005/01/11 21:18:01 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Thu Sep 12 17:25:22 1996 fingon
 * Last modified: Sun Sep 15 20:35:39 1996 fingon
 *
 */

#pragma once

#include "btech/context.h"
#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

typedef struct ArtilleryShot {
  int from_x, from_y; /* hex this is shot from */
  int to_x, to_y;     /* hex this lands in */
  int type;           /* weapon index in MechWeapons */
  int mode;           /* weapon mode */
  int ishit;          /* did we hit target hex? */
  DbRef shooter;      /* nice to know type of information */
  DbRef map;          /* map we're on */
  BtechContext *context;
} artillery_shot;

/* Weapon values for artillery guns */
constexpr int IS_LTOM = 30;
constexpr int IS_THUMPER = 31;
constexpr int IS_SNIPER = 32;
constexpr int IS_ARROW = 27;

constexpr int CL_ARROW = 71;
