
/*
   p.pcombat.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:04 CET 1999 from pcombat.c */

#pragma once

#include "mux/server/platform.h"

/* pcombat.c */
int personal_combat_damage_to_unit(Mech *target, int weapon_index, int damage);
int unit_damage_to_personal_combat(Mech *target, int weapon_index, int damage);
int personal_armor_reduce_damage(Mech *wounded, int cause, int hit_location,
                                 int internal_damage, int id);
