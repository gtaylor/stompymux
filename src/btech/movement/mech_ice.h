
/*
 * $Id: mech.ice.h,v 1.1.1.1 2005/01/11 21:18:17 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Thu Mar 20 20:37:15 1997 fingon
 * Last modified: Thu Mar 20 22:44:32 1997 fingon
 *
 */

#pragma once

typedef struct Mech Mech;

/* mech.ice.c */
void drop_thru_ice(Mech *mech);
void break_thru_ice(Mech *mech);
int possibly_drop_thru_ice(Mech *mech);
void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y);
void possibly_blow_ice(Mech *mech, int weapindx, int x, int y);
