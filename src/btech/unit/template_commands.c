#include "template_internal.h"

const char *load_cmds[] = {"Reference",
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
  return sizeof(load_cmds) / sizeof(*load_cmds) - 1;
}
