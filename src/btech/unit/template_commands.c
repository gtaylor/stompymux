#include "template_internal.h"
#include <stddef.h>

static const char *const LOAD_COMMAND_NAMES[] = {"Reference",
                                                 "Type",
                                                 "Move_Type",
                                                 "Tons",
                                                 "Tac_Range",
                                                 "LRS_Range",
                                                 "Radio_Range",
                                                 "Scan_Range",
                                                 "Heat_Sinks",
                                                 "Max_Speed",
                                                 "Specials",
                                                 "Armor",
                                                 "Internals",
                                                 "Rear",
                                                 "Config",
                                                 "Computer",
                                                 "Name",
                                                 "Jump_Speed",
                                                 "Radio",
                                                 "SI",
                                                 "Fuel",
                                                 "Comment",
                                                 "RadioType",
                                                 "Mech_BV",
                                                 "Cargo_Space",
                                                 "Max_Suits",
                                                 "InfantrySpecials",
                                                 "Max_Ton",
                                                 "HSEngOverRide",
                                                 "Unit_Era",
                                                 "Unit_TRO",
                                                 NULL};

size_t template_load_command_count(void) {
  return (sizeof(LOAD_COMMAND_NAMES) / sizeof(*LOAD_COMMAND_NAMES)) - 1;
}

const char *const *template_load_command_names(void) {
  return LOAD_COMMAND_NAMES;
}
