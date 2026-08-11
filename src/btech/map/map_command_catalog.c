#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition MAPCOMMANDS[] = {
    {0, "@VIEWXCODE", "@Views xcode values on object",
     btech_command_invoke_list_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@SETMAP <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},

    {0, "ADDICE <NUMBER>",
     "@Adds ice (<NUMBER> percent chance for each watery hex connected to "
     "land/ice)",
     btech_command_invoke_map_addice},
    {0, "DELICE <NUMBER>", "@Deletes first-melting ices at <NUMBER> chance",
     btech_command_invoke_map_delice},
    {0, "SETCOND <GRAV> <TEMP> [CLOUDBASE [VACUUM]]",
     "@Sets the map attributes (gravity: in 1/100'ths of Earth gravity, "
     "temperature: in Celsius, vacuum: optional, number (0 or 1)",
     btech_command_invoke_map_setconditions},
    {0, "VIEW <X> <Y>", "@Shows the map centered at X,Y",
     btech_command_invoke_map_view},

    {0, "ADDBLOCK <X> <Y> <DIST> [TEAM#_TO_ALLOW]",
     "@Adds no-landings zone of DIST hexes to X Y",
     btech_command_invoke_map_add_block},
    {0, "ADDMINE <X> <Y> <TYPE> <STRENGTH> [OPT]", "@Adds mine to X,Y",
     btech_command_invoke_mine_command_add},
    {0, "ADDHEX <X> <Y> <TERRAIN> <ELEV>",
     "@Changes the terrain and elevation of the given hex",
     btech_command_invoke_map_addhex},
    {0, "SETLINKED", "@Sets the map linked",
     btech_command_invoke_map_setlinked},
    {0, "@MAPEMIT <MESSAGE>", "@Emits stuff to the map",
     btech_command_invoke_map_mapemit},
    {0, "FIXMAP", "@Fixes inconsistencies in map",
     btech_command_invoke_debug_fixmap},
    {0, "LOADMAP <NAME>", "@Loads the named map",
     btech_command_invoke_map_loadmap},
    {0, "SAVEMAP <NAME>", "@Saves the map as name",
     btech_command_invoke_map_savemap},
    {0, "SETMAPSIZE <X> <Y>", "@Sets x and y size of map",
     btech_command_invoke_map_setmapsize},

    {0, "LIST [MECHS | OBJS]", "@Lists mechs/objects on the map",
     btech_command_invoke_map_listmechs},
    {0, "CLEARMECHS [DBNUM]", "@Clears mechs from the map",
     btech_command_invoke_map_clearmechs},
    {0, "ADDFIRE [X] [Y] [DURATION]", "@Adds fire that lasts <duration> secs",
     btech_command_invoke_map_addfire},
    {0, "ADDSMOKE [X] [Y] [DURATION]", "@Adds smoke that lasts <duration> secs",
     btech_command_invoke_map_addsmoke},
    {0, "DELOBJ [[TYPE] | [X] [Y] | [TYPE] [X] [Y]]",
     "@Deletes objects of either type or at x/y",
     btech_command_invoke_map_delobj},
    {0, "UPDATELINKS", "@Updates CodeLinks from the database objs (recursive)",
     btech_command_invoke_map_updatelinks},

    /* Cargo things */
    {0, "STORES", "Lists stuff in the hangar.",
     btech_command_invoke_mech_manifest},
    {0, "ADDSTUFF <NAME> <COUNT>", "@Adds <COUNT> <NAME> to map",
     btech_command_invoke_mech_raddstuff},

    {0, "FIXSTUFF", "@Fixes consistency errors in econ data",
     btech_command_invoke_mech_rfixstuff},
    {0, "REMOVESTUFF <NAME> <COUNT>", "@Removes <COUNT> <NAME> from map",
     btech_command_invoke_mech_rremovestuff},
    {0, "CLEARSTUFF", "@Removes all stuff from map",
     btech_command_invoke_mech_rresetstuff},
    {0, NULL, NULL, NULL}};

size_t map_command_count(void) {
  return sizeof(MAPCOMMANDS) / sizeof(*MAPCOMMANDS) - 1;
}
