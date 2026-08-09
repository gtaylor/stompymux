

/* Declares artillery combat interfaces. */

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
