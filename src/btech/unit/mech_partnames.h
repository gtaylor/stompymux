
/*
 * $Id: mech.partnames.h,v 1.1.1.1 2005/01/11 21:18:21 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Mar  9 14:02:41 1997 fingon
 * Last modified: Sat Jun  6 21:51:41 1998 fingon
 *
 */

#pragma once

#include "equipment_types.h"

typedef struct PartNameEntry {
  char *shorty;
  char *longy;
  char *vlongy;
  int index;
} PartNameEntry;

static inline int packed_part(int id, int brand) {
  return NUM_ITEMS * brand + id;
}

static inline int packed_part_id(int packed) { return packed % NUM_ITEMS; }

static inline int packed_part_brand(int packed) { return packed / NUM_ITEMS; }
