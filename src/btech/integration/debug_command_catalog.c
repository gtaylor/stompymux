#include "command_invokers.h"

const BtechCommandDefinition debugcommands[] = {
    {0, "EVENTSTATS", "@Shows event statistics",
     btech_command_invoke_debug_EventTypes},
    {0, "MEMSTATS [LONG]", "@Shows memory statistics (optionally in long form)",
     btech_command_invoke_debug_memory},
    {0, "SAVEDB", "@Writes a SQLite game checkpoint",
     btech_command_invoke_debug_savedb},
    {0, "LISTFORMS", "@Shows forms", btech_command_invoke_ListForms},

    {0, "SETVRT <WEAPON> <NUM>",
     "@Sets the VariableRecycleTime for weapon <WEAPON> to <NUM>",
     btech_command_invoke_debug_setvrt},
    {0, "SETXPLEVEL <SKILL> <NUM>",
     "@Sets the XP threshold for skill <skill> to <num>",
     btech_command_invoke_debug_setxplevel},
    {0, "SETWBV <WEAPON> <NUM>",
     "Sets the BattleValue for weapon <WEAPON> to <NUM",
     btech_command_invoke_debug_setwbv},
    {0, "SHUTDOWN <MAP#>", "@Shutdown all mechs on the map and clear it.",
     btech_command_invoke_debug_shutdown},

    {0, "XPTOP <SKILL>", "@Shows list of people champ in the <SKILL>",
     btech_command_invoke_debug_xptop},
    {0, NULL, NULL, NULL}};
