
/* Declares ice-related unit movement interfaces. */

#pragma once

typedef struct Mech Mech;

/* mech.ice.c */
void drop_thru_ice(Mech *mech);
void break_thru_ice(Mech *mech);
int possibly_drop_thru_ice(Mech *mech);
void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y);
void possibly_blow_ice(Mech *mech, int weapindx, int x, int y);
