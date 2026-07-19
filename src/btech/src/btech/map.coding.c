
/*
 * $Id: map.coding.c,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Tue Oct  8 16:46:12 1996 fingon
 * Last modified: Sat Jun  6 21:44:08 1998 fingon
 *
 */

/* Simple coding scheme to reduce space used by map hexes to 1 byte/hex */

/* NOTE: if we _ever_ use more than 255 terrain/elevation combinations,
   this code becomes a bomb. */

#include "map.coding.h"

#include "btech/map_coding_registry.h"

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation) {
  int i;

  if ((i = registry->data_to_id[(short)terrain][(short)elevation]))
    return i - 1;
  registry->id_to_data[registry->next_id] = (MapCodingEntry){
      .terrain = terrain,
      .elevation = elevation,
  };
  registry->next_id++;
  registry->data_to_id[(short)terrain][(short)elevation] = registry->next_id;
  return registry->next_id - 1;
}

char map_coding_get_elevation(const MapCodingRegistry *registry, int index) {
  return registry->id_to_data[index].elevation;
}

char map_coding_get_terrain(const MapCodingRegistry *registry, int index) {
  return registry->id_to_data[index].terrain;
}
