#include "aero_move_api.h"
#include "autopilot.h"
#include "bsuit_api.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "debug_api.h"
#include "ds_bay_api.h"
#include "ds_turret_api.h"
#include "eject_api.h"
#include "events_api.h"
#include "map_api.h"
#include "map_conditions_api.h"
#include "mech_advanced_api.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_combat_api.h"
#include "mech_consistency_api.h"
#include "mech_contacts_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_fire_api.h"
#include "mech_ice_api.h"
#include "mech_maps_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_pickup_api.h"
#include "mech_restrict_api.h"
#include "mech_scan_api.h"
#include "mech_sensor_api.h"
#include "mech_spot_api.h"
#include "mech_startup_api.h"
#include "mech_status_api.h"
#include "mech_tag_api.h"
#include "mech_tech.h"
#include "mech_tech_repairs_api.h"
#include "mech_tic_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mine_api.h"
#include "registry_internal.h"
#include "turret.h"
#include "value_handlers_api.h"

static void command_map_addice(DbRef actor, void *object, char *arguments) {
  map_addice(actor, object, arguments);
}

static void command_map_delice(DbRef actor, void *object, char *arguments) {
  map_delice(actor, object, arguments);
}

static void command_map_setconditions(DbRef actor, void *object,
                                      char *arguments) {
  map_setconditions(actor, object, arguments);
}

void newfreemech(DbRef, void **, int);

ECMD(f_mapblock_set);
ECMD(f_mapblock_setxy);
ECMD(ListForms);
ECMD(mech_ood_initiate);
ECMD(mech_Raddstuff);
ECMD(mech_Rfixstuff);
ECMD(mech_Rremovestuff);
ECMD(mech_Rresetstuff);
ECMD(mech_bomb);
ECMD(mech_loadcargo);
ECMD(mech_losemit);
ECMD(mech_manifest);
ECMD(mech_stores);
ECMD(mech_domystuff);
ECMD(mech_unloadcargo);
ECMD(tech_magic);
ECMD(tech_fixextra);
ECMD(mech_inferno);
ECMD(mech_swarm);
ECMD(mech_swarm1);
ECMD(mech_dig);
ECMD(mech_vector);
ECMD(mech_sguided);
ECMD(mech_atmrange);
ECMD(mech_atmexplosive);
ECMD(f_map_loadmap);

ECMD(f_draw);
ECMD(f_sheath);
ECMD(f_hold);
ECMD(f_put);

ECMD(f_shout);
ECMD(f_emote);
ECMD(f_say);

/* Flag:
 * 0 = ALL
 * 1 = MECH
 * 2 = GROUNDVEH
 * 4 = AERO
 * 8 = DS
 * 16 = VTOL
 * 32 = NAVAL
 * 64 = BSUIT
 * 128 = MW
 */

/* Categories:
   - Movement
   - Radio
   - Weapons
   - Physical
   - Status
   - Navigation
   - Repairing
   - Special
   - Information
   - TICs
   */

#define SHEADER(a, b) {a, b, b, NULL}
#define HEADER(a) SHEADER(0, a)

BtechCommandDefinition mapcommands[] = {
    {0, "@VIEWXCODE", "@Views xcode values on object", list_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@SETMAP <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},

    {0, "ADDICE <NUMBER>",
     "@Adds ice (<NUMBER> percent chance for each watery hex connected to "
     "land/ice)",
     command_map_addice},
    {0, "DELICE <NUMBER>", "@Deletes first-melting ices at <NUMBER> chance",
     command_map_delice},
    {0, "SETCOND <GRAV> <TEMP> [CLOUDBASE [VACUUM]]",
     "@Sets the map attributes (gravity: in 1/100'ths of Earth gravity, "
     "temperature: in Celsius, vacuum: optional, number (0 or 1)",
     command_map_setconditions},
    {0, "VIEW <X> <Y>", "@Shows the map centered at X,Y", map_view},

    {0, "ADDBLOCK <X> <Y> <DIST> [TEAM#_TO_ALLOW]",
     "@Adds no-landings zone of DIST hexes to X Y", map_add_block},
    {0, "ADDMINE <X> <Y> <TYPE> <STRENGTH> [OPT]", "@Adds mine to X,Y",
     mine_command_add},
    {0, "ADDHEX <X> <Y> <TERRAIN> <ELEV>",
     "@Changes the terrain and elevation of the given hex", map_addhex},
    {0, "SETLINKED", "@Sets the map linked", map_setlinked},
    {0, "@MAPEMIT <MESSAGE>", "@Emits stuff to the map", map_mapemit},
    {0, "FIXMAP", "@Fixes inconsistencies in map", debug_fixmap},
    {0, "LOADMAP <NAME>", "@Loads the named map", map_loadmap},
    {0, "SAVEMAP <NAME>", "@Saves the map as name", map_savemap},
    {0, "SETMAPSIZE <X> <Y>", "@Sets x and y size of map", map_setmapsize},

    {0, "LIST [MECHS | OBJS]", "@Lists mechs/objects on the map",
     map_listmechs},
    {0, "CLEARMECHS [DBNUM]", "@Clears mechs from the map", map_clearmechs},
    {0, "ADDFIRE [X] [Y] [DURATION]", "@Adds fire that lasts <duration> secs",
     map_addfire},
    {0, "ADDSMOKE [X] [Y] [DURATION]", "@Adds smoke that lasts <duration> secs",
     map_addsmoke},
    {0, "DELOBJ [[TYPE] | [X] [Y] | [TYPE] [X] [Y]]",
     "@Deletes objects of either type or at x/y", map_delobj},
    {0, "UPDATELINKS", "@Updates CodeLinks from the database objs (recursive)",
     map_updatelinks},

    /* Cargo things */
    {0, "STORES", "Lists stuff in the hangar.", mech_manifest},
    {0, "ADDSTUFF <NAME> <COUNT>", "@Adds <COUNT> <NAME> to map",
     mech_Raddstuff},

    {0, "FIXSTUFF", "@Fixes consistency errors in econ data", mech_Rfixstuff},
    {0, "REMOVESTUFF <NAME> <COUNT>", "@Removes <COUNT> <NAME> from map",
     mech_Rremovestuff},
    {0, "CLEARSTUFF", "@Removes all stuff from map", mech_Rresetstuff},
    {0, NULL, NULL, NULL}};

BtechCommandDefinition mechrepcommands[] = {
    {0, "SETTARGET <NUM>", "@Sets the mech to be repaired/built to num",
     mechrep_Rsettarget},

    {0, "LOADNEW <TYPENAME>", "@Loads a new mech template.", mechrep_Rloadnew},
    {0, "RESTORE", "@Completely repairs and reloads mech. ", mechrep_Rrestore},

    /* {0,"SAVENEW <TYPENAME>","@Saves the mech as a template.",
       mechrep_Rsavetemp}, */
    {0, "SAVENEW <TYPENAME>", "@Saves the mech as a new-type template.",
     mechrep_Rsavetemp2},
    {0, "SETARMOR <LOC> <AVAL> <IVAL> <RVAL>",
     "@Sets the armor, int. armor, and rear armor.", mechrep_Rsetarmor},
    {0, "ADDWEAP <NAME> <LOC> <CRIT SECS> [R|T|O]",
     "@Adds weapon to the mech, using given loc, crits, and flags.",
     mechrep_Raddweap},

    {0, "RESETCRITS", "@Resets criticals of the toy to base of type.",
     mechrep_Rresetcrits},
    {0, "REPAIR <LOC> <TYPE> <[VAL | SUBSECT]>", "@Repairs the mech.",
     mechrep_Rrepair},
    {0, "RELOAD <NAME> <LOC> <SUBSECT> [L|A|N(|C|M|S)]",
     "@Reloads weapon in location and critical subsection.", mechrep_Rreload},
    {0, "RESTOCK <LOC> <SUBSECT>",
     "@Simply restocks an ammo bin that's already present with the type of "
     "ammo it already had.",
     mechrep_Rrestock},
    {0, "FIREMODE <WEAP#> <MODE>", "@Changes firemode of weapon",
     mechrep_Rfiremode},
    {0, "ADDSP <ITEM> <LOC> <SUBSECT> [<DATA>]",
     "@Adds a special item in location & critical subsection.",
     mechrep_Raddspecial},
    {0, "DISPLAY <LOC>", "@Displays all the items in the location.",
     mechrep_Rdisplaysection},
    {0, "SHOWTECH", "@Shows the advanced technology of the mech.",
     mechrep_Rshowtech},
    {0, "ADDTECH <TYPE>", "@Adds the advanced technology to the mech.",
     mechrep_Raddtech},
    {0, "DELTECH <ALL or [<TECH>]>",
     "@Deletes all or one advanced technologies on the mech.",
     mechrep_Rdeltech},
    {0, "ADDINFTECH <TYPE>",
     "@Adds the advanced infantry technology to the mech.",
     mechrep_Raddinftech},
    {0, "DELINFTECH", "@Deletes the advanced infantry technology of the mech.",
     mechrep_Rdelinftech},
    {0, "SETTONS <NUM>", "@Sets the mech tonnage", mechrep_Rsettons},
    {0,
     "SETTYPE <MECH | GROUND | VTOL | NAVAL | AERO | DS | SPHEROIDDS | BSUIT >",
     "@Sets the mech type", mechrep_Rsettype},
    {0, "SETMOVE <TRACK | WHEEL | HOVER | VTOL | HULL | FOIL | FLY>",
     "@Sets the mech movement type", mechrep_Rsetmove},
    {0, "SETMAXSPEED <NUM>",
     "@Sets the max speed of the mech.  <NUM> is MP (i.e. SETMAXPSEED 6 for a "
     "4/6 unit)",
     mechrep_Rsetspeed},
    {0, "SETHEATSINKS <NUM>", "@Sets the number of heat sinks.",
     mechrep_Rsetheatsinks},
    {0, "SETJUMPSPEED <NUM>", "@Sets the jump speed of the mech.",
     mechrep_Rsetjumpspeed},
    {0, "SETLRSRANGE <NUM>", "@Sets the lrs range of the mech.",
     mechrep_Rsetlrsrange},
    {0, "SETTACRANGE <NUM>", "@Sets the tactical range of the mech.",
     mechrep_Rsettacrange},
    {0, "SETSCANRANGE <NUM>", "@Sets the scan range of the mech.",
     mechrep_Rsetscanrange},
    {0, "SETRADIO <NUM>", "@Sets the radio level of the mech.",
     mechrep_Rsetradio},
    {0, "SETRADIORANGE <NUM>", "@Sets the radio range of the mech.",
     mechrep_Rsetradiorange},
    {0, "SETCARGOSPACE <VAL> <MAXTON>",
     "@Sets cargospace and max cargo tonnage", mechrep_setcargospace},
    {0, NULL, NULL, NULL}};

BtechCommandDefinition autopilotcommands[] = {
    {0, "ENGAGE", "Engages the autopilot", auto_engage},
    {0, "DISENGAGE", "Disengages the autopilot", auto_disengage},

    {0, "ADDCOMMAND <NAME> [ARGS]", "Adds a command to queue", auto_addcommand},
    {0, "DELCOMMAND <NUM>", "Removes command <NUM> from queue (-1 = all)",
     auto_delcommand},
    {0, "LISTCOMMANDS", "Lists whole command queue of the autopilot",
     auto_listcommands},
    {0, "JUMP <NUM>", "Sets current instruction to <NUM>", auto_jump},
    {0, "EVENTSTATS", "Lists current events for this AI", auto_eventstats},
    {0, NULL, NULL, NULL}};

BtechCommandDefinition turretcommands[] = {
    {0, "@SETTURRET <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object", list_xcodestuff},
    {0, "DEINITIALIZE", "De-initializes you as gunner", turret_deinitialize},
    {0, "INITIALIZE", "Sets you as the gunner", turret_initialize},

    {0, "ADDTIC  <NUM> <WEAPNUM | LOWNUM-HIGHNUM>",
     "Adds weapnum, or lownum-highnum to given TIC", turret_addtic},
    {0, "BEARING [<X Y>] [<X Y>]", "Same format as range.", turret_bearing},
    {0, "CLEARTIC <NUM>", "Clears the TIC number given ", turret_cleartic},
    {0, "CONTACTS [<Prefix> | <TARGET-ID>]", "List all current contacts",
     turret_contacts},

    {0, "CRITSTATUS <SECTION>", "Shows the Critical hits status",
     turret_critstatus},
    {0, "DELTIC <NUM> <WEAPNUM>", "Deletes weapnum from given TIC",
     turret_deltic},
    {0, "ETA [<X> <Y>]", "Estimates time to target (/default target)",
     turret_eta},
    {0, "FINDCENTER", "Shows distance/bearing to center of hex.",
     turret_findcenter},
    {0, "FIRE <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Fires Weap at loc at def. target or specified target.",
     turret_fireweapon},
    {0, "FIRETIC <NUM> [<TARGET> or <X Y>]", "Fires the given TIC",
     turret_firetic},
    {0, "LISTTIC <NUM>", "Lists weapons in the given TIC", turret_listtic},

    {0, "LOCK [<TARGET-ID> | <X> <Y> | <X> <Y> <B|H> | -]",
     "Sets the target to the arg (in 3rd, B = building, H = hex "
     "(clear/ignite)) / Clears lock (-)",
     turret_settarget},
    {0, "LRS <M(ech) | T(errain) | E(lev)> [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the long range map", turret_lrsmap},
    {0, "NAVIGATE", "Shows the hex and surroundings graphically",
     turret_navigate},
    {0, "RANGE [<X Y>] [<X Y>]",
     "Range to def. target / range to x y / range to x,y from x,y",
     turret_range},
    {0, "REPORT [<TARGET-ID> | <X Y>]",
     "Information on default target, num, or x,y", turret_report},
    {0, "SCAN [<TARGET-ID> | <X Y> | <X Y> <B|H>]",
     "Scans the default target, chosen target, or hex", turret_scan},

    {0, "SIGHT <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Computes base-to-hit for given weapon and target.", turret_sight},
    {0, "STATUS [A(rmor)|I(nfo)]|W(eapons)|S(hort)]",
     "Prints the mech's status", turret_status},

    {0, "TACTICAL [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the tactical map at the mech's location / at bearing and range / "
     "around chosen target",
     turret_tacmap},
    {0, "WEAPONSPECS", "Shows the specifications for your weapons",
     turret_weaponspecs},
    {0, NULL, NULL, NULL}};

ECMD(debug_memory);
ECMD(debug_setvrt);
ECMD(debug_setxplevel);
ECMD(debug_setwbv);

BtechCommandDefinition debugcommands[] = {
    {0, "EVENTSTATS", "@Shows event statistics", debug_EventTypes},
    {0, "MEMSTATS [LONG]", "@Shows memory statistics (optionally in long form)",
     debug_memory},
    {0, "SAVEDB", "@Writes a SQLite game checkpoint", debug_savedb},
    {0, "LISTFORMS", "@Shows forms", ListForms},

    {0, "SETVRT <WEAPON> <NUM>",
     "@Sets the VariableRecycleTime for weapon <WEAPON> to <NUM>",
     debug_setvrt},
    {0, "SETXPLEVEL <SKILL> <NUM>",
     "@Sets the XP threshold for skill <skill> to <num>", debug_setxplevel},
    {0, "SETWBV <WEAPON> <NUM>",
     "Sets the BattleValue for weapon <WEAPON> to <NUM", debug_setwbv},
    {0, "SHUTDOWN <MAP#>", "@Shutdown all mechs on the map and clear it.",
     debug_shutdown},

    {0, "XPTOP <SKILL>", "@Shows list of people champ in the <SKILL>",
     debug_xptop},
    {0, NULL, NULL, NULL}};

BtechCommandDefinition sscommands[] = {
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object", list_xcodestuff},
    {0, NULL, NULL, NULL}};

#undef HEADER
