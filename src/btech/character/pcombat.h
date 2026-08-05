
/*
 * $Id: pcombat.h,v 1.1.1.1 2005/01/11 21:18:31 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Mar 23 20:11:57 1997 fingon
 * Last modified: Thu Aug 14 17:34:27 1997 fingon
 *
 */

#pragma once

typedef struct Mech Mech;

/* pcombat.c */
int personal_combat_damage_to_unit(Mech *target, int weapon_index, int damage);
int unit_damage_to_personal_combat(Mech *target, int weapon_index, int damage);
int personal_armor_reduce_damage(Mech *wounded, int cause, int hit_location,
                                 int internal_damage, int id);
