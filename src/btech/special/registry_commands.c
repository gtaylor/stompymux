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

static void command_aero_climb(DbRef actor, void *object, char *arguments) {
  aero_climb(actor, object, arguments);
}

static void command_aero_dive(DbRef actor, void *object, char *arguments) {
  aero_dive(actor, object, arguments);
}

static void command_mech_stinger(DbRef actor, void *object, char *arguments) {
  mech_stinger(actor, object, arguments);
}

static void command_mech_usebin(DbRef actor, void *object, char *arguments) {
  mech_usebin(actor, object, arguments);
}

static void command_mech_weaponstatus(DbRef actor, void *object,
                                      char *arguments) {
  mech_weaponstatus(actor, object, arguments);
}

static void command_aero_checklz(DbRef actor, void *object, char *arguments) {
  aero_checklz(actor, object, arguments);
}

static void command_mech_damage(DbRef actor, void *object, char *arguments) {
  mech_damage(actor, object, arguments);
}

static void command_mech_damage_section(DbRef actor, void *object,
                                        char *arguments) {
  mech_damage_section(actor, object, arguments);
}

static void command_mech_ecm(DbRef actor, void *object, char *arguments) {
  mech_ecm(actor, object, arguments);
}

static void command_mech_eccm(DbRef actor, void *object, char *arguments) {
  mech_eccm(actor, object, arguments);
}

static void command_mech_angelecm(DbRef actor, void *object, char *arguments) {
  mech_angelecm(actor, object, arguments);
}

static void command_mech_angeleccm(DbRef actor, void *object, char *arguments) {
  mech_angeleccm(actor, object, arguments);
}

static void command_mech_perecm(DbRef actor, void *object, char *arguments) {
  mech_perecm(actor, object, arguments);
}

static void command_mech_pereccm(DbRef actor, void *object, char *arguments) {
  mech_pereccm(actor, object, arguments);
}

static void command_mech_stealtharmor(DbRef actor, void *object,
                                      char *arguments) {
  mech_stealtharmor(actor, object, arguments);
}

static void command_mech_nullsig(DbRef actor, void *object, char *arguments) {
  mech_nullsig(actor, object, arguments);
}

static void command_mech_slite(DbRef actor, void *object, char *arguments) {
  mech_slite(actor, object, arguments);
}

static void command_mech_auto_turret(DbRef actor, void *object,
                                     char *arguments) {
  mech_auto_turret(actor, object, arguments);
}

static void command_vehicle_extinguish_fire(DbRef actor, void *object,
                                            char *arguments) {
  vehicle_extinquish_fire(actor, object, arguments);
}

static void command_mech_c3_message(DbRef actor, void *object,
                                    char *arguments) {
  mech_c3_message(actor, object, arguments);
}

static void command_mech_c3_targets(DbRef actor, void *object,
                                    char *arguments) {
  mech_c3_targets(actor, object, arguments);
}

static void command_mech_c3_network(DbRef actor, void *object,
                                    char *arguments) {
  mech_c3_network(actor, object, arguments);
}

static void command_mech_c3i_message(DbRef actor, void *object,
                                     char *arguments) {
  mech_c3i_message(actor, object, arguments);
}

static void command_mech_c3i_targets(DbRef actor, void *object,
                                     char *arguments) {
  mech_c3i_targets(actor, object, arguments);
}

static void command_mech_c3i_network(DbRef actor, void *object,
                                     char *arguments) {
  mech_c3i_network(actor, object, arguments);
}

static void command_show_narc_pods(DbRef actor, void *object, char *arguments) {
  show_narc_pods(actor, object, arguments);
}

static void command_remove_inarc_pods_mech(DbRef actor, void *object,
                                           char *arguments) {
  remove_inarc_pods_mech(actor, object, arguments);
}

static void command_remove_inarc_pods_tank(DbRef actor, void *object,
                                           char *arguments) {
  remove_inarc_pods_tank(actor, object, arguments);
}

static void command_tech_repairs(DbRef actor, void *object, char *arguments) {
  tech_repairs(actor, object, arguments);
}

static void command_mech_snipe(DbRef actor, void *object, char *arguments) {
  mech_snipe(actor, object, arguments);
}

void newfreemech(DbRef, void **, int);

ECMD(f_mapblock_set);
ECMD(f_mapblock_setxy);
ECMD(ListForms);
ECMD(initiate_ood);
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

BtechCommandDefinition mechcommands[] = {
    /* Movement */
    HEADER("Movement"),

    {0, "HEADING [num]",
     "Shows/Changes your heading to <NUM> (<NUM> in degrees)", mech_heading},
    {0, "SPEED [num | walk | run | stop | back | flank | cruise]",
     "Without arguments shows your present speed, otherwise changes your speed "
     "to <NUM> or the specified speed (run/cruise = 1x maxspeed, walk/flank = "
     "2/3x maxspeed, stop = 0, back = -2/3x maxspeed)",
     mech_speed},
    {48, "VERTICAL [num]", "Shows/Changes your vertical speed to <NUM>.",
     mech_vertical},

    {12, "CLIMB [angle]", "Shows/Changes the climbing angle to <NUM>.",
     command_aero_climb},
    {12, "DIVE [angle]", "Shows/Changes the diving angle to <NUM>.",
     command_aero_dive},
    {12, "THRUST [num]", "Shows/Changes the thrust to <NUM>.", aero_thrust},
    {0, "LATERAL [fl|fr|rl|rr|-]",
     "Change your lateral movement mode (quad only/vtol/hover, or with "
     "Maneuvering_ace). fl/fr/rl/rr = Directions, - = Disable lateral "
     "movement.",
     mech_lateral},
    {129, "STAND", "Stand up after a fall or dropping prone.", mech_stand},
    {1, "PRONE", "Force your 'mech to drop prone where it is.", mech_drop},
    {1, "THRASH", "Thrash around and try to kill nearby battle suits.",
     mech_thrash},
    {65, "JUMP [<TARGET-ID> | <BEARING> <RANGE>]",
     "Jump on default target / given target / bearing + range.", mech_jump},
    {1, "HULLDOWN [- | STOP]",
     "Allows a QUAD to go hull down behind a hill to gain better protection.",
     mech_hulldown},
    {0, "ENTERBASE [N|W|S|E]",
     "Enters base/hangar/whatnever from selected dir.", mech_enterbase},

    {0, "ENTERBAY [REF]",
     "Enters bay of a moving(?) hangar (DropShip for example). Ref is target "
     "ref, and it is optional.",
     mech_enterbay},
    {1, "BOOTLEGGER [R|L]",
     "Performs a bootlegger turn. This will turn you instantly 90 degrees in "
     "the desired direction, but requires a pilot roll. Roll BTH is based on "
     "tonnage and speed. Legs must not be recycling.",
     mech_bootlegger},
#ifdef BT_MOVEMENT_MODES
    {195, "SPRINT",
     "Toggles sprinting mode. While sprinting you are easier to hit, cannot "
     "attack, but can move 2xWalkSpeed.",
     mech_sprint},
    {195, "EVADE",
     "Toggles evasion mode. While evading you are harder to hit, but cannot "
     "attack.",
     mech_evade},
    {1, "DODGE",
     "Toggles dodge mode on. You must have Dodge_Maneuver advantage. While "
     "dodging you can counter physical attack rolls. One per turn.",
     mech_dodge},
#endif
    /* Radio */
    HEADER("Radio"),
    {0, "LISTCHANNELS", "Lists set frequencies + comtitles for them.",
     mech_list_freqs},
    {0, "SENDCHANNEL <LETTER> = <STRING>",
     "Sends <string> on channel <letter>'s freq.", mech_sendchannel},
    {0, "RADIO <ID> = <STRING>", "Radioes (LOS) <ID> with <STRING>",
     mech_radio},
    {0, "SETCHANNELFREQ <LETTER> = <NUMBER>",
     "Sets channel <letter> to frequency <number>.", mech_set_channelfreq},
    {0, "SETCHANNELMODE <LETTER> = <STRING>",
     "Sets channel <letter> mode <string> (available letters: DIUES, color "
     "codes).",
     mech_set_channelmode},
    {0, "SETCHANNELTITLE <LETTER> = <STRING>",
     "Sets channel <letter> comtitle to <string>.", mech_set_channeltitle},

    /* Weapons */
    HEADER("Weapons"),
    {0, "LOCK [<TARGET-ID> | <X> <Y> | <X> <Y> <B|H|I|C> | -]",
     "Sets the target to the (3rd argument :  B = building, C = clear, I = "
     "ignite, H = hex (clear/ignite/break ice/destroy bridge)) /, - = Clears "
     "present lock.",
     mech_settarget},
    {0, "SIGHT <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Computes base-to-hit for given weapon and target.", mech_sight},
    {0, "FIRE <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Fires weapon <weapnum> at def. target or specified target.",
     mech_fireweapon},
    {0, "TARGET <SECTION | ->",
     "Sets your aimed shot target / Disables targetting.", mech_target},
    {0, "TAG [ID|-]",
     "Lights an enemy unit with your TAG / Disables current TAG.", mech_tag},
    /* Weapon mode funcs */

    {0, "AMS <weapnum>", "Toggles Anti-Missile System on and off.", mech_ams},
    {0, "AP <weapnum>",
     "Sets/Unsets the autocannon to fire armor piercing rounds.",
     mech_armorpiercing},
    {0, "ARTEMIS <weapnum>", "Sets Weapon to and from ARTEMIS Mode.",
     mech_artemis},
    {0, "CASELESS <weaponum>", "Sets Weapons to and from CASELESS Mode.",
     mech_caseless},
    {0, "EXPLOSIVE <weapnum>", "Toggles between explosive/normal rounds",
     mech_explosive},
    {0, "FIRECLUSTER <weapnum>",
     "Sets/unsets artillery weapon to fire cluster rounds.", mech_cluster},
    {0, "FIREMINE <weapnum>",
     "Sets/unsets artillery weapon to fire mine rounds.", mech_mine},
    {0, "FIRESMOKE <weapnum>",
     "Sets/unsets artillery weapon to fire smoke rounds.", mech_smoke},
    {0, "FIRESWARM <weapnum>",
     "Sets/Unsets the LRM launcher to shoot swarm missiles", mech_swarm},
    {0, "FIRESWARM1 <weapnum>",
     "Sets/Unsets the LRM launcher to shoot swarm missiles", mech_swarm1},
    {0, "FLECHETTE <weapnum>",
     "Sets/Unsets the autocannon to fire flechette rounds.", mech_flechette},
    {0, "GATTLING <weapnum>",
     "Sets weapon to and from Gattling Mode (machineguns only).",
     mech_gattling},
    {0, "HEAT <weapnum>", "Sets a flamer to and from heat mode`.",
     mech_flamerheat},
    {0, "HOTLOAD <weapnum>",
     "Sets/Unsets the LRM launcher to hotload missiles, removing short-range "
     "penalties, but adding to chance of jamming.",
     mech_hotload},

    {0, "INARC <weapnum> <-|X|Y|E>",
     "Sets the type of ammo to fire from your iNarc weapon. '-': standard "
     "Homing, 'X': Explosive, 'Y': Haywire, 'E': ECM",
     mech_inarc_ammo_toggle},
    {0, "INCENDIARY <weapnum>",
     "Sets/Unsets the autocannon to fire incendiary rounds.", mech_incendiary},
    {0, "INFERNO <weapnum>",
     "Sets/Unsets the SRM launcher to shoot inferno missiles", mech_inferno},
    {0, "LBX <weapnum>", "Sets weapon to and from LBX Mode.", mech_lbx},
    {0, "NARC <weapnum>", "Sets weapon to and from NARC Mode.", mech_narc},
    {0, "SGUIDED <weapnum>", "Sets weapon to and from Sguide Mode",
     mech_sguided},
    {0, "STINGER <weaponum>", "Sets weapon to and from Stinger Mode.",
     command_mech_stinger},
    {0, "ATMRANGE <weapnum>", "Sets weapon to and from Extended Range Mode",
     mech_atmrange},
    {0, "ATMEXPLOSIVE <weapnum>", "Sets weapon to and from High Explosive Mode",
     mech_atmexplosive},
    {0, "PRECISION <weapnum>",
     "Sets/Unsets the autocannon to fire precision rounds.", mech_precision},
    {0, "RAC <weapnum> <-/2/4/6>",
     "Sets the Rotary AutoCannon to fire either 1, 2, 4 or 6 shots at a time.",
     mech_rac},
    {0, "RAPIDFIRE <weapnum>",
     "Sets weapon to and from Rapid Fire Mode (std and light ACs only).",
     mech_rapidfire},
    {0, "ULTRA <weapnum>", "Sets weapon to and from Ultra Mode.", mech_ultra},
    {0, "DISABLE <weapnum>", "Disables the weapon (Gauss only).",
     mech_disableweap},
    {0, "UNJAM <weapnum>", "Fixes ammo loader jams.", mech_unjamammo},
    {0, "USEBIN <weapnum> <location>",
     "Draw ammo from <location> first for <weapnum>.", command_mech_usebin},

    /* TIC Support */
    {0, "ADDTIC  <NUM> <WEAPNUM | LOWNUM-HIGHNUM>",
     "Adds weapon <weapnum>, or weapons from <lownum> to <highnum> to TIC "
     "<num>.",
     mech_addtic},
    {0, "CLEARTIC <NUM>", "Clears the TIC <num>.", mech_cleartic},

    {0, "DELTIC <NUM> <WEAPNUM>",
     "Deletes weapon number <weapnum> from TIC <num>.", mech_deltic},

    {0, "FIRETIC <NUM> [<TARGET> or <X Y>]", "Fires the weapons in TIC <num>.",
     mech_firetic},
    {0, "LISTTIC <NUM>", "Lists weapons in the TIC <num>.", mech_listtic},

    /* Information */
    HEADER("Information"),

    {0, "BRIEF [<LTR> <VAL>]",
     "Shows brief status / Sets brief for <ltr> to <val>.", mech_brief},
    {0, "CONTACTS [<Prefix> | <TARGET-ID>]", "List all current contacts",
     mech_contacts},

    {0, "CRITSTATUS <SECTION>", "Shows the Critical hits status",
     mech_critstatus},
    {0, "REPORT [<TARGET-ID> | <X Y>]",
     "Information on default target, num, or x,y", mech_report},
    {0, "SCAN [<TARGET-ID> | <X Y> | <X Y> <B|H>]",
     "Scans the default target, chosen target, or hex", mech_scan},

    {0, "SENSOR [LONG | [<V|L|I|E|S> <V|L|I|E|S>]]",
     "Shows/Changes your sensor mode (1 argument: Long, otherwise Short "
     "description about sensor mode)",
     mech_sensor},

    {0, "STATUS [A(rmor) | I(nfo)] | W(eapons)]", "Prints the mech's status",
     mech_status},
    {0, "VIEW [<TARGET-ID>]", "View the war painting on the target", mech_view},
    {0, "WEAPONSPECS", "Shows the specifications for your weapons",
     mech_weaponspecs},
    {0, "WEAPONSTATUS", "Shows the status of all your weapons",
     command_mech_weaponstatus},

    /* Navigation */
    HEADER("Navigation"),
    {0, "BEARING [<X Y>] [<X Y>]", "Same format as range.", mech_bearing},

    {0, "ETA [<X> <Y>]", "Estimates time to target (/default target)",
     mech_eta},
    {0, "FINDCENTER", "Shows distance/bearing to center of hex.",
     mech_findcenter},
    {0, "NAVIGATE", "Shows the hex and surroundings graphically",
     mech_navigate},
    {0, "RANGE [<X Y>] [<X Y>]",
     "Range to def. target / range to x y / range to x,y from x,y", mech_range},
    {0, "LRS <M|T|E|L|S|H|C> [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the (Mech/Terrain/Elevation/LOS/Sensors/Height/Combined) long "
     "range map",
     mech_lrsmap},
    {0, "TACTICAL [C | T | L] [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the tactical map at the mech's location / at bearing and range / "
     "around chosen target",
     mech_tacmap},
    {0, "VECTOR [<X Y> <X Y>]", "Same format as range.", mech_vector},

    /* Special */
    HEADER("Special"),

    {12, "CHECKLZ", "Checks if the landing-zone is good for a landing",
     command_aero_checklz},
    {0, "@OOD <X> <Y> [Z]",
     "@Initiates OOD drop at the orbit altitude to <X> <Y> (optional Z "
     "altitude to start from)",
     initiate_ood},
    {0, "@LOSEMIT <MESSAGE>",
     "@Sends message to everyone seeing the 'mech right now", mech_losemit},
    {0, "@DAMAGE <NUM> <CLUSTERSIZE> <ISREAR> <ISCRITICAL>",
     "@Causes <NUM> pt of damage to be done to 'mech in <CLUSTERSIZE> point "
     "clusters (if <ISREAR> is 1, damage is done to rear arc ; if <ISCRITICAL> "
     "is 1, damage does crit-thru-armor)",
     command_mech_damage},
    {0, "@DAMAGESECTION <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL>",
     "@Causes <DAMAGE> pts of damage to be done to unit's <SECTION>. <ISREAR> "
     "= 1 for REAR, <ISCRITICAL> = 1 for critical",
     command_mech_damage_section},
    {0, "@WEIGHT", "@Checks the weight allocated in the mech", mech_weight},
    {4, "BOMB [list | drop <num> | aim]",
     "Lists bombs / drops bomb <num> / aims where a bomb would fall.",
     mech_bomb},
    {2, "DIG", "Starts burrowing for cover [non-hovers only].", mech_dig},

    {2, "FIXTURRET",
     "Starts to fix a turret. Only works on jammed turrets, not locked "
     "turrets.",
     mech_fixturret},

    {0, "EXPLODE <AMMO|REACTOR|STOP>",
     "<AMMO|REACTOR> specifies which to ignite ; ammo causes all ammo on your "
     "mech to go *bang* (in no particular order), reactor disables control "
     "systems. Do note that neither are instant. STOP allows you to stop "
     "existing countdown.",
     mech_explode},
    {0, "SAFETY [ON|OFF]",
     "Enable/Disable Safeties against killing MechWarriors.", mech_safety},
    {0, "MECHPREFS [SETTING [ON|OFF]]",
     "Toggle the mechpref setting of SETTING", mech_mechprefs},
    {51, "TURNMODE [TIGHT | NORMAL]", "Sets turnmode.", mech_turnmode},

    /* Vengy's pickup/dropoff */
    {0, "DROPOFF", "Drops the mech you are carrying.", mech_dropoff},
    {0, "PICKUP [ID]", "Picks up [ID].", mech_pickup},
    {128, "ATTACHCABLES <ID1> <ID2>",
     "Attaches tow cables so that [ID1] can tow [ID2].", mech_attachcables},
    {128, "DETACHCABLES <ID>", "Detaches the tow cables from [ID].",
     mech_detachcables},

    {0, "DUMP <WEAPNUM|LOCATION|ALL|STOP> [<CRIT>]",
     "Dumps the ammunition for the weapon / in the location [ crit ] / all "
     "ammunition in the 'mech / stops all dumping in progress.",
     mech_dump},
    {1, "FLIPARMS", "Flips the arms to and from the rear arcs, if possible.",
     mech_fliparms},

    {0, "ECM",
     "Toggles the ECM status of your Guardian ECM suite (only applicable if "
     "you have one)",
     command_mech_ecm},
    {0, "ECCM",
     "Toggles the ECCM status of your Guardian ECM suite (only applicable if "
     "you have one)",
     command_mech_eccm},
    {0, "ANGELECM",
     "Toggles the ECM status of your Angel ECM suite (only applicable if you "
     "have one)",
     command_mech_angelecm},
    {0, "ANGELECCM",
     "Toggles the ECCM status of your Angel ECM suite (only applicable if you "
     "have one)",
     command_mech_angeleccm},

    {0, "PERECM",
     "Toggles the ECM status of your Personal ECM suite (only applicable if "
     "you have one)",
     command_mech_perecm},
    {0, "PERECCM",
     "Toggles the ECCM status of your Personal ECM suite (only applicable if "
     "you have one)",
     command_mech_pereccm},

    {0, "STEALTH",
     "Toggles status of Stealth Armor for those mechs equipped with it.",
     command_mech_stealtharmor},
    {0, "NSS",
     "Toggles status of the Null Signature System for those mechs equipped "
     "with it.",
     command_mech_nullsig},
    /* Ejection */

    {0, "DISEMBARK", "Gets the hell out of the 'mech / vehicle.",
     mech_disembark},
    {0, "UDISEMBARK", "Get the unit out of it's carrier.", mech_udisembark},
    {0, "EMBARK", "Climb into a 'mech / vehicle", mech_embark},
    {1, "MASC", "Toggles MASC on and off", mech_masc},
    {0, "SCHARGE", "Toggles Supercharger on and off", mech_scharge},
    /* DS / VTOL */
    {-2, "LAND", "Terminate your jump or land a VTOL/Aero/DS", mech_land},
    {-35, "TAKEOFF", "VTOL/Aero take off command", aero_takeoff},
    {1, "ROTTORSO <L(eft) | R(ight) | C(enter)>",
     "Rotates the torso 60 degrees right or left.", mech_rotatetorso},
    /* Nim's IDF things */
    {0, "SLITE", "Turns your searchlight on/off", command_mech_slite},

    {0, "SPOT [ID|-|OWNID]",
     "Sets someone as your spotter / makes you stop spotting / sets you as a "
     "spotter.",
     mech_spot},
    {0, "STARTUP [OVERRIDE]", "Commences startup cycle.", mech_startup},
    {0, "SHUTDOWN", "Shuts down the mech.", mech_shutdown},
    {34, "TURRET", "Set the turret facing.", mech_turret},
    {34, "AUTOTURRET", "Forces your turret to stay facing the locked target.",
     command_mech_auto_turret},
    {18, "EXTINGUISH",
     "Puts out the fires on your vehicle. You must be shut down to do this.",
     command_vehicle_extinguish_fire},

#ifdef C3_SUPPORT
    /* C3 */

    {0, "C3 [ID|-]",
     "Joins/Leaves a C3 network which the target mech is in. You will be "
     "assigned to a master computer within the network.",
     mech_c3_join_leave},
    {0, "C3MESSAGE <MSG>", "Sends a message to all others in your C3 network",
     command_mech_c3_message},
    {0, "C3TARGETS", "Shows available C3 targeting information",
     command_mech_c3_targets},
    {0, "C3NETWORK", "Displays information about your C3 network",
     command_mech_c3_network},

    {0, "C3I [ID|-]", "Joins/Leaves the C3i network connected to the target",
     mech_c3i_join_leave},
    {0, "C3IMESSAGE <MSG>", "Sends a message to all others in your C3i network",
     command_mech_c3i_message},
    {0, "C3ITARGETS", "Shows available C3i targeting information",
     command_mech_c3i_targets},
    {0, "C3INETWORK", "Displays information about your C3i network",
     command_mech_c3i_network},
#endif

    /* Heat stuff */

    {0, "HEATCUTOFF",
     "Sets your heat dissipation so that you wont go under 9 heat for TSM",
     heat_cutoff},
    {0, "PODS",
     "Shows the location of NARC and iNARC pods that attached to you",
     command_show_narc_pods},
    {1, "REMOVEPOD <LOCATION> <TYPE>",
     "Remove one of the pods from the selected location. Possible types are: "
     "'H' - Homing, 'Y' - Haywire, 'E' - ECM",
     command_remove_inarc_pods_mech},
    {18, "REMOVEPODS", "Removes all iNARC pods from the unit.",
     command_remove_inarc_pods_tank},

    /* Physical */
    SHEADER(1, "Physical"),
    {1, "AXE [R | L | B] [<TARGET-ID>]", "Axes a target", mech_axe},

    {3, "CHARGE [<TARGET-ID> | - ]",
     "Charges a target. '-' removes charge command.", mech_charge},
    {1, "CHOP [R | L | B] [<TARGET-ID>]", "Chops target with a sword",
     mech_sword},
    {1, "CLAW [R | L |B] [<TARGET-ID>]", "Claws a target", mech_claw},
    {1, "CLUB [<TARGET-ID>]", "Clubs a target with a tree", mech_club},
    {1, "MACE [<TARGET-ID>]", "Maces your target", mech_mace},
    {1, "SAW [<TARGET-ID>]", "Saws a target with a Dual Saw", mech_saw},
    {1, "KICK [R | L] [<TARGET-ID>]", "Kicks a target", mech_kick},
    {1, "TRIP [R | L] [<TARGET-ID>]", "Trips a target mech", mech_trip},
    {1, "PUNCH [R | L | B] [<TARGET-ID>]", "Punches a target", mech_punch},
    {1, "GRABCLUB [R | L | -]", "Grabs a tree and carries it around as a club",
     mech_grabclub},

    {64, "ATTACKLEG [<TARGET-ID>]", "Attacks legs of the target battlemech",
     bsuit_attackleg},
    {0, "HIDE",
     "Attempts to hide your team ; doesn't work if any hostiles have their "
     "eyeballs on you",
     bsuit_hide},
    {64, "SWARM [<TARGET-ID> | -]", "Swarms the target / drop off target (-)",
     bsuit_swarm},
    {64, "JETTISON", "Jettison your backpack", JettisonPacks},

    /* Repairing */
    HEADER("Repair"),
    {0, "CHECKSTATUS", "Checks mech's techstatus", tech_checkstatus},
    {0, "DAMAGES", "Shows the mech's damages", show_mechs_damage},
    {0, "FIX [<NUM> | <LOW-HI>]", "Fixes entry <NUM> from mech's damages",
     tech_fix},
    {0, "FIXARMOR <LOC>", "Repairs armor in <loc>", tech_fixarmor},
    {0, "FIXINTERNAL <LOC>", "Repairs internals in <loc>", tech_fixinternal},
    {0, "REATTACH <LOC>", "Reattaches the limb", tech_reattach},
    {64, "REPLACESUIT <SUIT>", "Replaces the missing suit", tech_replacesuit},
    {0, "RESEAL <LOC>", "Reseals the limb", tech_reseal},

    {0, "RELOAD <LOC> <POS> [TYPE]",
     "Reloads the ammo compartment in <loc>/<pos> (optionally with [type])",
     tech_reload},
    {0, "TOGGLETYPE <loc> <pos> <type>",
     "Set the type of ammo in ammobin <loc>/<pos> to type <type>",
     tech_toggletype},
    {0, "REMOVEGUN <NUM>", "Removes the gun", tech_removegun},
    {0, "REMOVEPART <LOC> <POS>", "Removes the part", tech_removepart},
    {0, "REMOVESECTION <LOC>", "Removes the section", tech_removesection},

    {0, "REPLACEGUN [<NUM> | <LOC> <POS>] [ITEM]",
     "Replaces the gun in the position (optionally with [ITEM], like "
     "Martell.MediumLaser)",
     tech_replacegun},

    {0, "REPAIRGUN [<NUM> | <LOC> <POS>]", "Repairs the gun in the position",
     tech_repairgun},
    {0, "REPLACEPART <LOC> <POS>", "Replaces the part in the position",
     tech_replacepart},
    {0, "REPAIRPART <LOC> <POS>", "Repairs the part in the position",
     tech_repairpart},
    {0, "REPAIRS", "Shows repairs/scrapping in progress", command_tech_repairs},
    {0, "UNLOAD <LOC> <POS>", "Unloads the ammo compartment in <loc>/<pos>",
     tech_unload},

    {0, "@MAGIC", "@Fixes the unfixable - skirt crits etc (wiz-only)",
     tech_magic},
    {0, "@FIXEXTRA", "@fixes extra stuff, like reseal, ammo feeds, etc",
     tech_fixextra},

#ifdef BT_CARGO_COMMANDS
    /* Cargo */
    HEADER("Cargo"),

    {0, "LOADCARGO <NAME> <COUNT>", "Loads up <COUNT> <NAME>s from the bay.",
     mech_loadcargo},
    {0, "MANIFEST", "Lists stuff carried by mech.", mech_manifest},
    {0, "STORES", "Lists stuff in the bay.", mech_stores},
    {0, "UNLOADCARGO <NAME> <COUNT>", "Unloads <COUNT> <NAME>s to the bay.",
     mech_unloadcargo},
#endif

    /* Restricted commands */
    HEADER("@Restricted"),
    {0, "@CREATEBAYS [.. list of DBrefs, seperated by space]",
     "@Creates / Disables bays on a DS", mech_createbays},
    {0, "@SETMECH <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object", list_xcodestuff},

    {0, "SNIPE <ID> <WEAPON>",
     "@Lets you 'snipe' (=shoot artillery weapons with movement prediction)",
     command_mech_snipe},
    {0, "ADDSTUFF <NAME> <COUNT>", "@Adds <COUNT> <NAME> to mech's inventory",
     mech_Raddstuff},

    {0, "FIXSTUFF", "@Fixes consistency errors in econ data", mech_Rfixstuff},
    {0, "CLEARSTUFF", "@Removes all stuff from 'mech", mech_Rresetstuff},

    {0, "REMOVESTUFF <NAME> <COUNT>",
     "@Removes <COUNT> <NAME> from mech's inventory", mech_Rremovestuff},
    {0, "SETMAPINDX <NUM>", "@Sets the mech's map index to num.",
     mech_Rsetmapindex},
    {0, "SETTEAM <NUM>", "@Sets the teams.", mech_Rsetteam},
    {0, "SETXY <X> <Y>", "@Sets the x & y value of the mech.", mech_Rsetxy},
    {0, NULL, NULL, NULL}};

ECMD(map_addice);
ECMD(map_delice);
ECMD(map_setconditions);
