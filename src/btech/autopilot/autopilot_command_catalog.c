#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition autopilotcommands[] = {
    {0, "ENGAGE", "Engages the autopilot", btech_command_invoke_auto_engage},
    {0, "DISENGAGE", "Disengages the autopilot",
     btech_command_invoke_auto_disengage},

    {0, "ADDCOMMAND <NAME> [ARGS]", "Adds a command to queue",
     btech_command_invoke_auto_addcommand},
    {0, "DELCOMMAND <NUM>", "Removes command <NUM> from queue (-1 = all)",
     btech_command_invoke_auto_delcommand},
    {0, "LISTCOMMANDS", "Lists whole command queue of the autopilot",
     btech_command_invoke_auto_listcommands},
    {0, "JUMP <NUM>", "Sets current instruction to <NUM>",
     btech_command_invoke_auto_jump},
    {0, "EVENTSTATS", "Lists current events for this AI",
     btech_command_invoke_auto_eventstats},
    {0, NULL, NULL, NULL}};

size_t autopilot_command_count(void) {
  return sizeof(autopilotcommands) / sizeof(*autopilotcommands) - 1;
}
