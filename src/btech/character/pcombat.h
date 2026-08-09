
/* Declares personnel combat interfaces. */

#pragma once

typedef struct Mech Mech;

/* pcombat.c */
int personal_combat_damage_to_unit(Mech *target, int weapon_index, int damage);
int unit_damage_to_personal_combat(Mech *target, int weapon_index, int damage);
int personal_armor_reduce_damage(Mech *wounded, int cause, int hit_location,
                                 int internal_damage, int id);
