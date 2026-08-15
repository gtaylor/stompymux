#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition MECHREPCOMMANDS[] = {
    {0, "SETTARGET <NUM>", "@Sets the mech to be repaired/built to num",
     btech_command_invoke_mechrep_rsettarget},

    {0, "LOADNEW <TYPENAME>", "@Loads a new mech template.",
     btech_command_invoke_mechrep_rloadnew},
    {0, "RESTORE", "@Completely repairs and reloads mech. ",
     btech_command_invoke_mechrep_rrestore},

    /* {0,"SAVENEW <TYPENAME>","@Saves the mech as a template.",
       btech_command_invoke_mechrep_rsavetemp}, */
    {0, "SAVENEW <TYPENAME>", "@Saves the mech as a new-type template.",
     btech_command_invoke_mechrep_rsavetemp2},
    {0, "SETARMOR <LOC> <AVAL> <IVAL> <RVAL>",
     "@Sets the armor, int. armor, and rear armor.",
     btech_command_invoke_mechrep_rsetarmor},
    {0, "ADDWEAP <NAME> <LOC> <CRIT SECS> [R|T|O]",
     "@Adds weapon to the mech, using given loc, crits, and flags.",
     btech_command_invoke_mechrep_raddweap},

    {0, "RESETCRITS", "@Resets criticals of the toy to base of type.",
     btech_command_invoke_mechrep_rresetcrits},
    {0, "REPAIR <LOC> <TYPE> <[VAL | SUBSECT]>", "@Repairs the mech.",
     btech_command_invoke_mechrep_rrepair},
    {0, "RELOAD <NAME> <LOC> <SUBSECT> [L|A|N(|C|M|S)]",
     "@Reloads weapon in location and critical subsection.",
     btech_command_invoke_mechrep_rreload},
    {0, "RESTOCK <LOC> <SUBSECT>",
     "@Simply restocks an ammo bin that's already present with the type of "
     "ammo it already had.",
     btech_command_invoke_mechrep_rrestock},
    {0, "FIREMODE <WEAP#> <MODE>", "@Changes firemode of weapon",
     btech_command_invoke_mechrep_rfiremode},
    {0, "ADDSP <ITEM> <LOC> <SUBSECT> [<DATA>]",
     "@Adds a special item in location & critical subsection.",
     btech_command_invoke_mechrep_raddspecial},
    {0, "DISPLAY <LOC>", "@Displays all the items in the location.",
     btech_command_invoke_mechrep_rdisplaysection},
    {0, "SHOWTECH", "@Shows the advanced technology of the mech.",
     btech_command_invoke_mechrep_rshowtech},
    {0, "ADDTECH <TYPE>", "@Adds the advanced technology to the mech.",
     btech_command_invoke_mechrep_raddtech},
    {0, "DELTECH <ALL or [<TECH>]>",
     "@Deletes all or one advanced technologies on the mech.",
     btech_command_invoke_mechrep_rdeltech},
    {0, "ADDINFTECH <TYPE>",
     "@Adds the advanced infantry technology to the mech.",
     btech_command_invoke_mechrep_raddinftech},
    {0, "DELINFTECH", "@Deletes the advanced infantry technology of the mech.",
     btech_command_invoke_mechrep_rdelinftech},
    {0, "SETTONS <NUM>", "@Sets the mech tonnage",
     btech_command_invoke_mechrep_rsettons},
    {0,
     "SETTYPE <MECH | GROUND | VTOL | NAVAL | AERO | DS | SPHEROIDDS | BSUIT >",
     "@Sets the mech type", btech_command_invoke_mechrep_rsettype},
    {0, "SETMOVE <TRACK | WHEEL | HOVER | VTOL | HULL | FOIL | FLY>",
     "@Sets the mech movement type", btech_command_invoke_mechrep_rsetmove},
    {0, "SETMAXSPEED <NUM>",
     "@Sets the max speed of the mech.  <NUM> is MP (i.e. SETMAXPSEED 6 for a "
     "4/6 unit)",
     btech_command_invoke_mechrep_rsetspeed},
    {0, "SETHEATSINKS <NUM>", "@Sets the number of heat sinks.",
     btech_command_invoke_mechrep_rsetheatsinks},
    {0, "SETJUMPSPEED <NUM>", "@Sets the jump speed of the mech.",
     btech_command_invoke_mechrep_rsetjumpspeed},
    {0, "SETLRSRANGE <NUM>", "@Sets the lrs range of the mech.",
     btech_command_invoke_mechrep_rsetlrsrange},
    {0, "SETTACRANGE <NUM>", "@Sets the tactical range of the mech.",
     btech_command_invoke_mechrep_rsettacrange},
    {0, "SETSCANRANGE <NUM>", "@Sets the scan range of the mech.",
     btech_command_invoke_mechrep_rsetscanrange},
    {0, "SETRADIO <NUM>", "@Sets the radio level of the mech.",
     btech_command_invoke_mechrep_rsetradio},
    {0, "SETRADIORANGE <NUM>", "@Sets the radio range of the mech.",
     btech_command_invoke_mechrep_rsetradiorange},
    {0, "SETCARGOSPACE <VAL> <MAXTON>",
     "@Sets cargospace and max cargo tonnage",
     btech_command_invoke_mechrep_setcargospace},
    {0, nullptr, nullptr, nullptr}};

size_t repair_command_count(void) {
  return (sizeof(MECHREPCOMMANDS) / sizeof(*MECHREPCOMMANDS)) - 1;
}
