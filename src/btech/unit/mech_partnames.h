
/* Declares names for unit parts and locations. */

#pragma once

#include "equipment_types.h"

typedef struct PartNameEntry {
  char *shorty;
  char *longy;
  char *vlongy;
  int index;
} PartNameEntry;

static inline int packed_part(int id, int brand) {
  return (NUM_ITEMS * brand) + id;
}

static inline int packed_part_id(int packed) { return packed % NUM_ITEMS; }

static inline int packed_part_brand(int packed) { return packed / NUM_ITEMS; }
