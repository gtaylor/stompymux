/* Converts registered part-name entries to public part identities. */

#include "mech_partnames.h"
#include "mech_partnames_api.h"

PartReference part_name_reference(const PartNameEntry *entry) {
  if (entry == nullptr)
    return (PartReference){.id = -1, .brand = -1};
  return (PartReference){.id = packed_part_id(entry->index),
                         .brand = packed_part_brand(entry->index)};
}
