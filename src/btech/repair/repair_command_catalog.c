#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition mechrepcommands[] = {
    {0, "SETTARGET <NUM>", "@Sets the mech to be repaired/built to num",
     btech_command_invoke_mechrep_Rsettarget},

    {0, "LOADNEW <TYPENAME>", "@Loads a new mech template.",
     btech_command_invoke_mechrep_Rloadnew},
    {0, "RESTORE", "@Completely repairs and reloads mech. ",
     btech_command_invoke_mechrep_Rrestore},

    /* {0,"SAVENEW <TYPENAME>","@Saves the mech as a template.",
       btech_command_invoke_mechrep_Rsavetemp}, */
    {0, "SAVENEW <TYPENAME>", "@Saves the mech as a new-type template.",
     btech_command_invoke_mechrep_Rsavetemp2},
    {0, "SETARMOR <LOC> <AVAL> <IVAL> <RVAL>",
     "@Sets the armor, int. armor, and rear armor.",
     btech_command_invoke_mechrep_Rsetarmor},
    {0, "ADDWEAP <NAME> <LOC> <CRIT SECS> [R|T|O]",
     "@Adds weapon to the mech, using given loc, crits, and flags.",
     btech_command_invoke_mechrep_Raddweap},

    {0, "RESETCRITS", "@Resets criticals of the toy to base of type.",
     btech_command_invoke_mechrep_Rresetcrits},
    {0, "REPAIR <LOC> <TYPE> <[VAL | SUBSECT]>", "@Repairs the mech.",
     btech_command_invoke_mechrep_Rrepair},
    {0, "RELOAD <NAME> <LOC> <SUBSECT> [L|A|N(|C|M|S)]",
     "@Reloads weapon in location and critical subsection.",
     btech_command_invoke_mechrep_Rreload},
    {0, "RESTOCK <LOC> <SUBSECT>",
     "@Simply restocks an ammo bin that's already present with the type of "
     "ammo it already had.",
     btech_command_invoke_mechrep_Rrestock},
    {0, "FIREMODE <WEAP#> <MODE>", "@Changes firemode of weapon",
     btech_command_invoke_mechrep_Rfiremode},
    {0, "ADDSP <ITEM> <LOC> <SUBSECT> [<DATA>]",
     "@Adds a special item in location & critical subsection.",
     btech_command_invoke_mechrep_Raddspecial},
    {0, "DISPLAY <LOC>", "@Displays all the items in the location.",
     btech_command_invoke_mechrep_Rdisplaysection},
    {0, "SHOWTECH", "@Shows the advanced technology of the mech.",
     btech_command_invoke_mechrep_Rshowtech},
    {0, "ADDTECH <TYPE>", "@Adds the advanced technology to the mech.",
     btech_command_invoke_mechrep_Raddtech},
    {0, "DELTECH <ALL or [<TECH>]>",
     "@Deletes all or one advanced technologies on the mech.",
     btech_command_invoke_mechrep_Rdeltech},
    {0, "ADDINFTECH <TYPE>",
     "@Adds the advanced infantry technology to the mech.",
     btech_command_invoke_mechrep_Raddinftech},
    {0, "DELINFTECH", "@Deletes the advanced infantry technology of the mech.",
     btech_command_invoke_mechrep_Rdelinftech},
    {0, "SETTONS <NUM>", "@Sets the mech tonnage",
     btech_command_invoke_mechrep_Rsettons},
    {0,
     "SETTYPE <MECH | GROUND | VTOL | NAVAL | AERO | DS | SPHEROIDDS | BSUIT >",
     "@Sets the mech type", btech_command_invoke_mechrep_Rsettype},
    {0, "SETMOVE <TRACK | WHEEL | HOVER | VTOL | HULL | FOIL | FLY>",
     "@Sets the mech movement type", btech_command_invoke_mechrep_Rsetmove},
    {0, "SETMAXSPEED <NUM>",
     "@Sets the max speed of the mech.  <NUM> is MP (i.e. SETMAXPSEED 6 for a "
     "4/6 unit)",
     btech_command_invoke_mechrep_Rsetspeed},
    {0, "SETHEATSINKS <NUM>", "@Sets the number of heat sinks.",
     btech_command_invoke_mechrep_Rsetheatsinks},
    {0, "SETJUMPSPEED <NUM>", "@Sets the jump speed of the mech.",
     btech_command_invoke_mechrep_Rsetjumpspeed},
    {0, "SETLRSRANGE <NUM>", "@Sets the lrs range of the mech.",
     btech_command_invoke_mechrep_Rsetlrsrange},
    {0, "SETTACRANGE <NUM>", "@Sets the tactical range of the mech.",
     btech_command_invoke_mechrep_Rsettacrange},
    {0, "SETSCANRANGE <NUM>", "@Sets the scan range of the mech.",
     btech_command_invoke_mechrep_Rsetscanrange},
    {0, "SETRADIO <NUM>", "@Sets the radio level of the mech.",
     btech_command_invoke_mechrep_Rsetradio},
    {0, "SETRADIORANGE <NUM>", "@Sets the radio range of the mech.",
     btech_command_invoke_mechrep_Rsetradiorange},
    {0, "SETCARGOSPACE <VAL> <MAXTON>",
     "@Sets cargospace and max cargo tonnage",
     btech_command_invoke_mechrep_setcargospace},
    {0, NULL, NULL, NULL}};

size_t repair_command_count(void) {
  return sizeof(mechrepcommands) / sizeof(*mechrepcommands) - 1;
}
