#include "equipment_types.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mux/support/alloc.h"
#include "section_types.h"
#include "template_api.h"
#include "template_internal.h"

#include <stdio.h>
#include <string.h>

const char *const *primary_technology_abbreviations(void) {
  static const char *const ABBREVIATIONS[] = {""};
  return ABBREVIATIONS;
}

const char *const *secondary_technology_abbreviations(void) {
  static const char *const ABBREVIATIONS[] = {""};
  return ABBREVIATIONS;
}

const char *const *infantry_technology_abbreviations(void) {
  static const char *const ABBREVIATIONS[] = {""};
  return ABBREVIATIONS;
}

size_t primary_technology_name_count(void) { return 0; }
size_t secondary_technology_name_count(void) { return 0; }
size_t infantry_technology_name_count(void) { return 0; }

char *template_bit_string_build(const TemplateBitStringRequest *request) {
  request->buffer[0] = '\0';
  return request->buffer;
}

int mech_critical_part_type(const Mech *mech [[maybe_unused]], int section,
                            int critical) {
  return section == 0 && critical == 0 ? special_equipment_index(AXE) : 0;
}

bool mech_critical_is_operational_special(const CriticalSpecialCheck *check
                                          [[maybe_unused]]) {
  return false;
}

bool mech_section_configuration_has(const Mech *mech [[maybe_unused]],
                                    int section [[maybe_unused]],
                                    int configuration [[maybe_unused]]) {
  return false;
}

int main(void) {
  Mech mech = {};
  char buffer[MBUF_SIZE];

  /* Avoid the biped towing checks so this test isolates tech-list appends. */
  mech.ud.type = CLASS_MECH;
  mech.ud.move = MOVE_QUAD;
  if (techlist_func(&mech, buffer, sizeof(buffer)) != buffer) {
    fputs("techlist_func returned a different buffer\n", stderr);
    return 1;
  }
  if (strstr(buffer, " AXE") == nullptr) {
    fprintf(stderr, "tech list omitted AXE: '%s'\n", buffer);
    return 2;
  }
  return 0;
}
