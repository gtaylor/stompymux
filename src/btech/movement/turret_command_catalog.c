#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition TURRETCOMMANDS[] = {
    {0, "@SETTURRET <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object",
     btech_command_invoke_list_xcodestuff},
    {0, "DEINITIALIZE", "De-initializes you as gunner",
     btech_command_invoke_turret_deinitialize},
    {0, "INITIALIZE", "Sets you as the gunner",
     btech_command_invoke_turret_initialize},

    {0, "ADDTIC  <NUM> <WEAPNUM | LOWNUM-HIGHNUM>",
     "Adds weapnum, or lownum-highnum to given TIC",
     btech_command_invoke_turret_addtic},
    {0, "BEARING [<X Y>] [<X Y>]", "Same format as range.",
     btech_command_invoke_turret_bearing},
    {0, "CLEARTIC <NUM>", "Clears the TIC number given ",
     btech_command_invoke_turret_cleartic},
    {0, "CONTACTS [<Prefix> | <TARGET-ID>]", "List all current contacts",
     btech_command_invoke_turret_contacts},

    {0, "CRITSTATUS <SECTION>", "Shows the Critical hits status",
     btech_command_invoke_turret_critstatus},
    {0, "DELTIC <NUM> <WEAPNUM>", "Deletes weapnum from given TIC",
     btech_command_invoke_turret_deltic},
    {0, "ETA [<X> <Y>]", "Estimates time to target (/default target)",
     btech_command_invoke_turret_eta},
    {0, "FINDCENTER", "Shows distance/bearing to center of hex.",
     btech_command_invoke_turret_findcenter},
    {0, "FIRE <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Fires Weap at loc at def. target or specified target.",
     btech_command_invoke_turret_fireweapon},
    {0, "FIRETIC <NUM> [<TARGET> or <X Y>]", "Fires the given TIC",
     btech_command_invoke_turret_firetic},
    {0, "LISTTIC <NUM>", "Lists weapons in the given TIC",
     btech_command_invoke_turret_listtic},

    {0, "LOCK [<TARGET-ID> | <X> <Y> | <X> <Y> <B|H> | -]",
     "Sets the target to the arg (in 3rd, B = building, H = hex "
     "(clear/ignite)) / Clears lock (-)",
     btech_command_invoke_turret_settarget},
    {0, "LRS <M(ech) | T(errain) | E(lev)> [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the long range map", btech_command_invoke_turret_lrsmap},
    {0, "NAVIGATE", "Shows the hex and surroundings graphically",
     btech_command_invoke_turret_navigate},
    {0, "RANGE [<X Y>] [<X Y>]",
     "Range to def. target / range to x y / range to x,y from x,y",
     btech_command_invoke_turret_range},
    {0, "REPORT [<TARGET-ID> | <X Y>]",
     "Information on default target, num, or x,y",
     btech_command_invoke_turret_report},
    {0, "SCAN [<TARGET-ID> | <X Y> | <X Y> <B|H>]",
     "Scans the default target, chosen target, or hex",
     btech_command_invoke_turret_scan},

    {0, "SIGHT <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Computes base-to-hit for given weapon and target.",
     btech_command_invoke_turret_sight},
    {0, "STATUS [A(rmor)|I(nfo)]|W(eapons)|S(hort)]",
     "Prints the mech's status", btech_command_invoke_turret_status},

    {0, "TACTICAL [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the tactical map at the mech's location / at bearing and range / "
     "around chosen target",
     btech_command_invoke_turret_tacmap},
    {0, "WEAPONSPECS", "Shows the specifications for your weapons",
     btech_command_invoke_turret_weaponspecs},
    {0, nullptr, nullptr, nullptr}};

size_t turret_command_count(void) {
  return (sizeof(TURRETCOMMANDS) / sizeof(*TURRETCOMMANDS)) - 1;
}
