
/*
   p.failures.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:41 CET 1999 from failures.c */

#pragma once

#include "mux/server/platform.h"

#include "mech_api_types.h"

const char *mech_part_brand_name(int type, int level);
void mech_generic_failure_check(Mech *mech, int type, int *result,
                                int *modifier);
void mech_weapon_failure_check(Mech *mech, int weapon_number, int weapon_type,
                               int section, int critical, int *modifier,
                               int *type);
