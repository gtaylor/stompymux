#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"
#include <stddef.h>

const BtechCommandDefinition MECHCOMMANDS[] = {
    /* Movement */
    {0, "Movement", "Movement", nullptr},

    {0, "HEADING [num]",
     "Shows/Changes your heading to <NUM> (<NUM> in degrees)",
     btech_command_invoke_mech_heading},
    {0, "SPEED [num | walk | run | stop | back | flank | cruise]",
     "Without arguments shows your present speed, otherwise changes your speed "
     "to <NUM> or the specified speed (run/cruise = 1x maxspeed, walk/flank = "
     "2/3x maxspeed, stop = 0, back = -2/3x maxspeed)",
     btech_command_invoke_mech_speed},
    {48, "VERTICAL [num]", "Shows/Changes your vertical speed to <NUM>.",
     btech_command_invoke_mech_vertical},

    {12, "CLIMB [angle]", "Shows/Changes the climbing angle to <NUM>.",
     btech_command_invoke_aero_climb},
    {12, "DIVE [angle]", "Shows/Changes the diving angle to <NUM>.",
     btech_command_invoke_aero_dive},
    {12, "THRUST [num]", "Shows/Changes the thrust to <NUM>.",
     btech_command_invoke_aero_thrust},
    {0, "LATERAL [fl|fr|rl|rr|-]",
     "Change your lateral movement mode (quad only/vtol/hover, or with "
     "Maneuvering_ace). fl/fr/rl/rr = Directions, - = Disable lateral "
     "movement.",
     btech_command_invoke_mech_lateral},
    {129, "STAND", "Stand up after a fall or dropping prone.",
     btech_command_invoke_mech_stand},
    {1, "PRONE", "Force your 'mech to drop prone where it is.",
     btech_command_invoke_mech_drop},
    {1, "THRASH", "Thrash around and try to kill nearby battle suits.",
     btech_command_invoke_mech_thrash},
    {65, "JUMP [<TARGET-ID> | <BEARING> <RANGE>]",
     "Jump on default target / given target / bearing + range.",
     btech_command_invoke_mech_jump},
    {1, "HULLDOWN [- | STOP]",
     "Allows a QUAD to go hull down behind a hill to gain better protection.",
     btech_command_invoke_mech_hulldown},
    {0, "ENTERBASE [N|W|S|E]",
     "Enters base/hangar/whatnever from selected dir.",
     btech_command_invoke_mech_enterbase},

    {0, "ENTERBAY [REF]",
     "Enters bay of a moving(?) hangar (DropShip for example). Ref is target "
     "ref, and it is optional.",
     btech_command_invoke_mech_enterbay},
    {1, "BOOTLEGGER [R|L]",
     "Performs a bootlegger turn. This will turn you instantly 90 degrees in "
     "the desired direction, but requires a pilot roll. Roll BTH is based on "
     "tonnage and speed. Legs must not be recycling.",
     btech_command_invoke_mech_bootlegger},
#ifdef BT_MOVEMENT_MODES
    {195, "SPRINT",
     "Toggles sprinting mode. While sprinting you are easier to hit, cannot "
     "attack, but can move 2xWalkSpeed.",
     btech_command_invoke_mech_sprint},
    {195, "EVADE",
     "Toggles evasion mode. While evading you are harder to hit, but cannot "
     "attack.",
     btech_command_invoke_mech_evade},
    {1, "DODGE",
     "Toggles dodge mode on. You must have Dodge_Maneuver advantage. While "
     "dodging you can counter physical attack rolls. One per turn.",
     btech_command_invoke_mech_dodge},
#endif
    /* Radio */
    {0, "Radio", "Radio", nullptr},
    {0, "LISTCHANNELS", "Lists set frequencies + comtitles for them.",
     btech_command_invoke_mech_list_freqs},
    {0, "SENDCHANNEL <LETTER> = <STRING>",
     "Sends <string> on channel <letter>'s freq.",
     btech_command_invoke_mech_sendchannel},
    {0, "RADIO <ID> = <STRING>", "Radioes (LOS) <ID> with <STRING>",
     btech_command_invoke_mech_radio},
    {0, "SETCHANNELFREQ <LETTER> = <NUMBER>",
     "Sets channel <letter> to frequency <number>.",
     btech_command_invoke_mech_set_channelfreq},
    {0, "SETCHANNELMODE <LETTER> = <STRING>",
     "Sets channel <letter> mode <string> (available letters: DIUES, color "
     "codes).",
     btech_command_invoke_mech_set_channelmode},
    {0, "SETCHANNELTITLE <LETTER> = <STRING>",
     "Sets channel <letter> comtitle to <string>.",
     btech_command_invoke_mech_set_channeltitle},

    /* Weapons */
    {0, "Weapons", "Weapons", nullptr},
    {0, "LOCK [<TARGET-ID> | <X> <Y> | <X> <Y> <B|H|I|C> | -]",
     "Sets the target to the (3rd argument :  B = building, C = clear, I = "
     "ignite, H = hex (clear/ignite/break ice/destroy bridge)) /, - = Clears "
     "present lock.",
     btech_command_invoke_mech_set_target},
    {0, "SIGHT <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Computes base-to-hit for given weapon and target.",
     btech_command_invoke_mech_sight},
    {0, "FIRE <WEAPNUM> [<TARGET-ID> | <X> <Y>]",
     "Fires weapon <weapnum> at def. target or specified target.",
     btech_command_invoke_mech_fireweapon},
    {0, "TARGET <SECTION | ->",
     "Sets your aimed shot target / Disables targetting.",
     btech_command_invoke_mech_target},
    {0, "TAG [ID|-]",
     "Lights an enemy unit with your TAG / Disables current TAG.",
     btech_command_invoke_mech_tag},
    /* Weapon mode funcs */

    {0, "AMS <weapnum>", "Toggles Anti-Missile System on and off.",
     btech_command_invoke_mech_ams},
    {0, "AP <weapnum>",
     "Sets/Unsets the autocannon to fire armor piercing rounds.",
     btech_command_invoke_mech_armorpiercing},
    {0, "ARTEMIS <weapnum>", "Sets Weapon to and from ARTEMIS Mode.",
     btech_command_invoke_mech_artemis},
    {0, "CASELESS <weaponum>", "Sets Weapons to and from CASELESS Mode.",
     btech_command_invoke_mech_caseless},
    {0, "EXPLOSIVE <weapnum>", "Toggles between explosive/normal rounds",
     btech_command_invoke_mech_explosive},
    {0, "FIRECLUSTER <weapnum>",
     "Sets/unsets artillery weapon to fire cluster rounds.",
     btech_command_invoke_mech_cluster},
    {0, "FIREMINE <weapnum>",
     "Sets/unsets artillery weapon to fire mine rounds.",
     btech_command_invoke_mech_mine},
    {0, "FIRESMOKE <weapnum>",
     "Sets/unsets artillery weapon to fire smoke rounds.",
     btech_command_invoke_mech_smoke},
    {0, "FIRESWARM <weapnum>",
     "Sets/Unsets the LRM launcher to shoot swarm missiles",
     btech_command_invoke_mech_swarm},
    {0, "FIRESWARM1 <weapnum>",
     "Sets/Unsets the LRM launcher to shoot swarm missiles",
     btech_command_invoke_mech_swarm1},
    {0, "FLECHETTE <weapnum>",
     "Sets/Unsets the autocannon to fire flechette rounds.",
     btech_command_invoke_mech_flechette},
    {0, "GATTLING <weapnum>",
     "Sets weapon to and from Gattling Mode (machineguns only).",
     btech_command_invoke_mech_gattling},
    {0, "HEAT <weapnum>", "Sets a flamer to and from heat mode`.",
     btech_command_invoke_mech_flamerheat},
    {0, "HOTLOAD <weapnum>",
     "Sets/Unsets the LRM launcher to hotload missiles, removing short-range "
     "penalties, but adding to chance of jamming.",
     btech_command_invoke_mech_hotload},

    {0, "INARC <weapnum> <-|X|Y|E>",
     "Sets the type of ammo to fire from your iNarc weapon. '-': standard "
     "Homing, 'X': Explosive, 'Y': Haywire, 'E': ECM",
     btech_command_invoke_mech_inarc_ammo_toggle},
    {0, "INCENDIARY <weapnum>",
     "Sets/Unsets the autocannon to fire incendiary rounds.",
     btech_command_invoke_mech_incendiary},
    {0, "INFERNO <weapnum>",
     "Sets/Unsets the SRM launcher to shoot inferno missiles",
     btech_command_invoke_mech_inferno},
    {0, "LBX <weapnum>", "Sets weapon to and from LBX Mode.",
     btech_command_invoke_mech_lbx},
    {0, "NARC <weapnum>", "Sets weapon to and from NARC Mode.",
     btech_command_invoke_mech_narc},
    {0, "SGUIDED <weapnum>", "Sets weapon to and from Sguide Mode",
     btech_command_invoke_mech_sguided},
    {0, "STINGER <weaponum>", "Sets weapon to and from Stinger Mode.",
     btech_command_invoke_mech_stinger},
    {0, "ATMRANGE <weapnum>", "Sets weapon to and from Extended Range Mode",
     btech_command_invoke_mech_atmrange},
    {0, "ATMEXPLOSIVE <weapnum>", "Sets weapon to and from High Explosive Mode",
     btech_command_invoke_mech_atmexplosive},
    {0, "PRECISION <weapnum>",
     "Sets/Unsets the autocannon to fire precision rounds.",
     btech_command_invoke_mech_precision},
    {0, "RAC <weapnum> <-/2/4/6>",
     "Sets the Rotary AutoCannon to fire either 1, 2, 4 or 6 shots at a time.",
     btech_command_invoke_mech_rac},
    {0, "RAPIDFIRE <weapnum>",
     "Sets weapon to and from Rapid Fire Mode (std and light ACs only).",
     btech_command_invoke_mech_rapidfire},
    {0, "ULTRA <weapnum>", "Sets weapon to and from Ultra Mode.",
     btech_command_invoke_mech_ultra},
    {0, "DISABLE <weapnum>", "Disables the weapon (Gauss only).",
     btech_command_invoke_mech_disableweap},
    {0, "UNJAM <weapnum>", "Fixes ammo loader jams.",
     btech_command_invoke_mech_unjamammo},
    {0, "USEBIN <weapnum> <location>",
     "Draw ammo from <location> first for <weapnum>.",
     btech_command_invoke_mech_usebin},

    /* TIC Support */
    {0, "ADDTIC  <NUM> <WEAPNUM | LOWNUM-HIGHNUM>",
     "Adds weapon <weapnum>, or weapons from <lownum> to <highnum> to TIC "
     "<num>.",
     btech_command_invoke_mech_addtic},
    {0, "CLEARTIC <NUM>", "Clears the TIC <num>.",
     btech_command_invoke_mech_cleartic},

    {0, "DELTIC <NUM> <WEAPNUM>",
     "Deletes weapon number <weapnum> from TIC <num>.",
     btech_command_invoke_mech_deltic},

    {0, "FIRETIC <NUM> [<TARGET> or <X Y>]", "Fires the weapons in TIC <num>.",
     btech_command_invoke_mech_firetic},
    {0, "LISTTIC <NUM>", "Lists weapons in the TIC <num>.",
     btech_command_invoke_mech_listtic},

    /* Information */
    {0, "Information", "Information", nullptr},

    {0, "BRIEF [<LTR> <VAL>]",
     "Shows brief status / Sets brief for <ltr> to <val>.",
     btech_command_invoke_mech_brief},
    {0, "CONTACTS [<Prefix> | <TARGET-ID>]", "List all current contacts",
     btech_command_invoke_mech_contacts},

    {0, "CRITSTATUS <SECTION>", "Shows the Critical hits status",
     btech_command_invoke_mech_critstatus},
    {0, "REPORT [<TARGET-ID> | <X Y>]",
     "Information on default target, num, or x,y",
     btech_command_invoke_mech_report},
    {0, "SCAN [<TARGET-ID> | <X Y> | <X Y> <B|H>]",
     "Scans the default target, chosen target, or hex",
     btech_command_invoke_mech_scan},

    {0, "SENSOR [LONG | [<V|L|I|E|S> <V|L|I|E|S>]]",
     "Shows/Changes your sensor mode (1 argument: Long, otherwise Short "
     "description about sensor mode)",
     btech_command_invoke_mech_sensor},

    {0, "STATUS [A(rmor) | I(nfo)] | W(eapons)]", "Prints the mech's status",
     btech_command_invoke_mech_status},
    {0, "VIEW [<TARGET-ID>]", "View the war painting on the target",
     btech_command_invoke_mech_view},
    {0, "WEAPONSPECS", "Shows the specifications for your weapons",
     btech_command_invoke_mech_weaponspecs},
    {0, "WEAPONSTATUS", "Shows the status of all your weapons",
     btech_command_invoke_mech_weapon_status},

    /* Navigation */
    {0, "Navigation", "Navigation", nullptr},
    {0, "BEARING [<X Y>] [<X Y>]", "Same format as range.",
     btech_command_invoke_mech_bearing},

    {0, "ETA [<X> <Y>]", "Estimates time to target (/default target)",
     btech_command_invoke_mech_eta},
    {0, "FINDCENTER", "Shows distance/bearing to center of hex.",
     btech_command_invoke_mech_findcenter},
    {0, "NAVIGATE", "Shows the hex and surroundings graphically",
     btech_command_invoke_mech_navigate},
    {0, "RANGE [<X Y>] [<X Y>]",
     "Range to def. target / range to x y / range to x,y from x,y",
     btech_command_invoke_mech_range},
    {0, "LRS <M|T|E|L|S|H|C> [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the (Mech/Terrain/Elevation/LOS/Sensors/Height/Combined) long "
     "range map",
     btech_command_invoke_mech_lrsmap},
    {0, "TACTICAL [C | T | L] [<BEARING> <RANGE> | <TARGET-ID>]",
     "Shows the tactical map at the mech's location / at bearing and range / "
     "around chosen target",
     btech_command_invoke_mech_tacmap},
    {0, "VECTOR [<X Y> <X Y>]", "Same format as range.",
     btech_command_invoke_mech_vector},

    /* Special */
    {0, "Special", "Special", nullptr},

    {12, "CHECKLZ", "Checks if the landing-zone is good for a landing",
     btech_command_invoke_aero_checklz},
    {0, "@OOD <X> <Y> [Z]",
     "@Initiates OOD drop at the orbit altitude to <X> <Y> (optional Z "
     "altitude to start from)",
     btech_command_invoke_mech_ood_initiate},
    {0, "@LOSEMIT <MESSAGE>",
     "@Sends message to everyone seeing the 'mech right now",
     btech_command_invoke_mech_losemit},
    {0, "@DAMAGE <NUM> <CLUSTERSIZE> <ISREAR> <ISCRITICAL>",
     "@Causes <NUM> pt of damage to be done to 'mech in <CLUSTERSIZE> point "
     "clusters (if <ISREAR> is 1, damage is done to rear arc ; if <ISCRITICAL> "
     "is 1, damage does crit-thru-armor)",
     btech_command_invoke_mech_damage},
    {0, "@DAMAGESECTION <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL>",
     "@Causes <DAMAGE> pts of damage to be done to unit's <SECTION>. <ISREAR> "
     "= 1 for REAR, <ISCRITICAL> = 1 for critical",
     btech_command_invoke_mech_damage_section},
    {0, "@WEIGHT", "@Checks the weight allocated in the mech",
     btech_command_invoke_mech_weight},
    {4, "BOMB [list | drop <num> | aim]",
     "Lists bombs / drops bomb <num> / aims where a bomb would fall.",
     btech_command_invoke_mech_bomb},
    {2, "DIG", "Starts burrowing for cover [non-hovers only].",
     btech_command_invoke_mech_dig},

    {2, "FIXTURRET",
     "Starts to fix a turret. Only works on jammed turrets, not locked "
     "turrets.",
     btech_command_invoke_mech_fixturret},

    {0, "EXPLODE <AMMO|REACTOR|STOP>",
     "<AMMO|REACTOR> specifies which to ignite ; ammo causes all ammo on your "
     "mech to go *bang* (in no particular order), reactor disables control "
     "systems. Do note that neither are instant. STOP allows you to stop "
     "existing countdown.",
     btech_command_invoke_mech_explode},
    {0, "SAFETY [ON|OFF]",
     "Enable/Disable Safeties against killing MechWarriors.",
     btech_command_invoke_mech_safety},
    {0, "MECHPREFS [SETTING [ON|OFF]]",
     "Toggle the mechpref setting of SETTING",
     btech_command_invoke_mech_mechprefs},
    {51, "TURNMODE [TIGHT | NORMAL]", "Sets turnmode.",
     btech_command_invoke_mech_turnmode},

    /* Vengy's pickup/dropoff */
    {0, "DROPOFF", "Drops the mech you are carrying.",
     btech_command_invoke_mech_dropoff},
    {0, "PICKUP [ID]", "Picks up [ID].", btech_command_invoke_mech_pickup},
    {128, "ATTACHCABLES <ID1> <ID2>",
     "Attaches tow cables so that [ID1] can tow [ID2].",
     btech_command_invoke_mech_attachcables},
    {128, "DETACHCABLES <ID>", "Detaches the tow cables from [ID].",
     btech_command_invoke_mech_detachcables},

    {0, "DUMP <WEAPNUM|LOCATION|ALL|STOP> [<CRIT>]",
     "Dumps the ammunition for the weapon / in the location [ crit ] / all "
     "ammunition in the 'mech / stops all dumping in progress.",
     btech_command_invoke_mech_dump},
    {1, "FLIPARMS", "Flips the arms to and from the rear arcs, if possible.",
     btech_command_invoke_mech_fliparms},

    {0, "ECM",
     "Toggles the ECM status of your Guardian ECM suite (only applicable if "
     "you have one)",
     btech_command_invoke_mech_ecm},
    {0, "ECCM",
     "Toggles the ECCM status of your Guardian ECM suite (only applicable if "
     "you have one)",
     btech_command_invoke_mech_eccm},
    {0, "ANGELECM",
     "Toggles the ECM status of your Angel ECM suite (only applicable if you "
     "have one)",
     btech_command_invoke_mech_angelecm},
    {0, "ANGELECCM",
     "Toggles the ECCM status of your Angel ECM suite (only applicable if you "
     "have one)",
     btech_command_invoke_mech_angeleccm},

    {0, "PERECM",
     "Toggles the ECM status of your Personal ECM suite (only applicable if "
     "you have one)",
     btech_command_invoke_mech_perecm},
    {0, "PERECCM",
     "Toggles the ECCM status of your Personal ECM suite (only applicable if "
     "you have one)",
     btech_command_invoke_mech_pereccm},

    {0, "STEALTH",
     "Toggles status of Stealth Armor for those mechs equipped with it.",
     btech_command_invoke_mech_stealtharmor},
    {0, "NSS",
     "Toggles status of the Null Signature System for those mechs equipped "
     "with it.",
     btech_command_invoke_mech_nullsig},
    /* Ejection */

    {0, "DISEMBARK", "Gets the hell out of the 'mech / vehicle.",
     btech_command_invoke_mech_disembark},
    {0, "UDISEMBARK", "Get the unit out of it's carrier.",
     btech_command_invoke_mech_udisembark},
    {0, "EMBARK", "Climb into a 'mech / vehicle",
     btech_command_invoke_mech_embark},
    {1, "MASC", "Toggles MASC on and off", btech_command_invoke_mech_masc},
    {0, "SCHARGE", "Toggles Supercharger on and off",
     btech_command_invoke_mech_scharge},
    /* DS / VTOL */
    {-2, "LAND", "Terminate your jump or land a VTOL/Aero/DS",
     btech_command_invoke_mech_land},
    {-35, "TAKEOFF", "VTOL/Aero take off command",
     btech_command_invoke_aero_takeoff},
    {1, "ROTTORSO <L(eft) | R(ight) | C(enter)>",
     "Rotates the torso 60 degrees right or left.",
     btech_command_invoke_mech_rotatetorso},
    /* Nim's IDF things */
    {0, "SLITE", "Turns your searchlight on/off",
     btech_command_invoke_mech_slite},

    {0, "SPOT [ID|-|OWNID]",
     "Sets someone as your spotter / makes you stop spotting / sets you as a "
     "spotter.",
     btech_command_invoke_mech_spot},
    {0, "STARTUP [OVERRIDE]", "Commences startup cycle.",
     btech_command_invoke_mech_startup},
    {0, "SHUTDOWN", "Shuts down the mech.", btech_command_invoke_mech_shutdown},
    {34, "TURRET", "Set the turret facing.", btech_command_invoke_mech_turret},
    {34, "AUTOTURRET", "Forces your turret to stay facing the locked target.",
     btech_command_invoke_mech_auto_turret},
    {18, "EXTINGUISH",
     "Puts out the fires on your vehicle. You must be shut down to do this.",
     btech_command_invoke_vehicle_fire_extinguish},

#ifdef C3_SUPPORT
    /* C3 */

    {0, "C3 [ID|-]",
     "Joins/Leaves a C3 network which the target mech is in. You will be "
     "assigned to a master computer within the network.",
     btech_command_invoke_mech_c3_join_leave},
    {0, "C3MESSAGE <MSG>", "Sends a message to all others in your C3 network",
     btech_command_invoke_mech_c3_message},
    {0, "C3TARGETS", "Shows available C3 targeting information",
     btech_command_invoke_mech_c3_targets},
    {0, "C3NETWORK", "Displays information about your C3 network",
     btech_command_invoke_mech_c3_network},

    {0, "C3I [ID|-]", "Joins/Leaves the C3i network connected to the target",
     btech_command_invoke_mech_c3i_join_leave},
    {0, "C3IMESSAGE <MSG>", "Sends a message to all others in your C3i network",
     btech_command_invoke_mech_c3i_message},
    {0, "C3ITARGETS", "Shows available C3i targeting information",
     btech_command_invoke_mech_c3i_targets},
    {0, "C3INETWORK", "Displays information about your C3i network",
     btech_command_invoke_mech_c3i_network},
#endif

    /* Heat stuff */

    {0, "HEATCUTOFF",
     "Sets your heat dissipation so that you wont go under 9 heat for TSM",
     btech_command_invoke_heat_cutoff},
    {0, "PODS",
     "Shows the location of NARC and iNARC pods that attached to you",
     btech_command_invoke_show_narc_pods},
    {1, "REMOVEPOD <LOCATION> <TYPE>",
     "Remove one of the pods from the selected location. Possible types are: "
     "'H' - Homing, 'Y' - Haywire, 'E' - ECM",
     btech_command_invoke_remove_inarc_pods_mech},
    {18, "REMOVEPODS", "Removes all iNARC pods from the unit.",
     btech_command_invoke_remove_inarc_pods_tank},

    /* Physical */
    {1, "Physical", "Physical", nullptr},
    {1, "AXE [R | L | B] [<TARGET-ID>]", "Axes a target",
     btech_command_invoke_mech_axe},

    {3, "CHARGE [<TARGET-ID> | - ]",
     "Charges a target. '-' removes charge command.",
     btech_command_invoke_mech_charge},
    {1, "CHOP [R | L | B] [<TARGET-ID>]", "Chops target with a sword",
     btech_command_invoke_mech_sword},
    {1, "CLAW [R | L |B] [<TARGET-ID>]", "Claws a target",
     btech_command_invoke_mech_claw},
    {1, "CLUB [<TARGET-ID>]", "Clubs a target with a tree",
     btech_command_invoke_mech_club},
    {1, "MACE [<TARGET-ID>]", "Maces your target",
     btech_command_invoke_mech_mace},
    {1, "SAW [<TARGET-ID>]", "Saws a target with a Dual Saw",
     btech_command_invoke_mech_saw},
    {1, "KICK [R | L] [<TARGET-ID>]", "Kicks a target",
     btech_command_invoke_mech_kick},
    {1, "TRIP [R | L] [<TARGET-ID>]", "Trips a target mech",
     btech_command_invoke_mech_trip},
    {1, "PUNCH [R | L | B] [<TARGET-ID>]", "Punches a target",
     btech_command_invoke_mech_punch},
    {1, "GRABCLUB [R | L | -]", "Grabs a tree and carries it around as a club",
     btech_command_invoke_mech_grabclub},

    {64, "ATTACKLEG [<TARGET-ID>]", "Attacks legs of the target battlemech",
     btech_command_invoke_bsuit_attackleg},
    {0, "HIDE",
     "Attempts to hide your team ; doesn't work if any hostiles have their "
     "eyeballs on you",
     btech_command_invoke_bsuit_hide},
    {64, "SWARM [<TARGET-ID> | -]", "Swarms the target / drop off target (-)",
     btech_command_invoke_bsuit_swarm},
    {64, "JETTISON", "Jettison your backpack",
     btech_command_invoke_bsuit_pack_jettison},

    /* Repairing */
    {0, "Repair", "Repair", nullptr},
    {0, "CHECKSTATUS", "Checks mech's techstatus",
     btech_command_invoke_tech_checkstatus},
    {0, "DAMAGES", "Shows the mech's damages",
     btech_command_invoke_show_mechs_damage},
    {0, "FIX [<NUM> | <LOW-HI>]", "Fixes entry <NUM> from mech's damages",
     btech_command_invoke_tech_fix},
    {0, "FIXARMOR <LOC>", "Repairs armor in <loc>",
     btech_command_invoke_tech_fixarmor},
    {0, "FIXINTERNAL <LOC>", "Repairs internals in <loc>",
     btech_command_invoke_tech_fixinternal},
    {0, "REATTACH <LOC>", "Reattaches the limb",
     btech_command_invoke_tech_reattach},
    {64, "REPLACESUIT <SUIT>", "Replaces the missing suit",
     btech_command_invoke_tech_replacesuit},
    {0, "RESEAL <LOC>", "Reseals the limb", btech_command_invoke_tech_reseal},

    {0, "RELOAD <LOC> <POS> [TYPE]",
     "Reloads the ammo compartment in <loc>/<pos> (optionally with [type])",
     btech_command_invoke_tech_reload},
    {0, "TOGGLETYPE <loc> <pos> <type>",
     "Set the type of ammo in ammobin <loc>/<pos> to type <type>",
     btech_command_invoke_tech_toggletype},
    {0, "REMOVEGUN <NUM>", "Removes the gun",
     btech_command_invoke_tech_removegun},
    {0, "REMOVEPART <LOC> <POS>", "Removes the part",
     btech_command_invoke_tech_removepart},
    {0, "REMOVESECTION <LOC>", "Removes the section",
     btech_command_invoke_tech_removesection},

    {0, "REPLACEGUN [<NUM> | <LOC> <POS>] [ITEM]",
     "Replaces the gun in the position (optionally with [ITEM], like "
     "Martell.MediumLaser)",
     btech_command_invoke_tech_replacegun},

    {0, "REPAIRGUN [<NUM> | <LOC> <POS>]", "Repairs the gun in the position",
     btech_command_invoke_tech_repairgun},
    {0, "REPLACEPART <LOC> <POS>", "Replaces the part in the position",
     btech_command_invoke_tech_replacepart},
    {0, "REPAIRPART <LOC> <POS>", "Repairs the part in the position",
     btech_command_invoke_tech_repairpart},
    {0, "REPAIRS", "Shows repairs/scrapping in progress",
     btech_command_invoke_tech_repairs},
    {0, "UNLOAD <LOC> <POS>", "Unloads the ammo compartment in <loc>/<pos>",
     btech_command_invoke_tech_unload},

    {0, "@MAGIC", "@Fixes the unfixable - skirt crits etc (wiz-only)",
     btech_command_invoke_tech_magic},
    {0, "@FIXEXTRA", "@fixes extra stuff, like reseal, ammo feeds, etc",
     btech_command_invoke_tech_fixextra},

    /* Cargo */
    {0, "Cargo", "Cargo", nullptr},

    {0, "LOADCARGO <NAME> <COUNT>", "Loads up <COUNT> <NAME>s from the bay.",
     btech_command_invoke_mech_loadcargo},
    {0, "MANIFEST", "Lists stuff carried by mech.",
     btech_command_invoke_mech_manifest},
    {0, "STORES", "Lists stuff in the bay.", btech_command_invoke_mech_stores},
    {0, "UNLOADCARGO <NAME> <COUNT>", "Unloads <COUNT> <NAME>s to the bay.",
     btech_command_invoke_mech_unloadcargo},

    /* Restricted commands */
    {0, "@Restricted", "@Restricted", nullptr},
    {0, "@CREATEBAYS [.. list of DBrefs, seperated by space]",
     "@Creates / Disables bays on a DS", btech_command_invoke_mech_createbays},
    {0, "@SETMECH <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object",
     btech_command_invoke_list_xcodestuff},

    {0, "SNIPE <ID> <WEAPON>",
     "@Lets you 'snipe' (=shoot artillery weapons with movement prediction)",
     btech_command_invoke_mech_snipe},
    {0, "ADDSTUFF <NAME> <COUNT>", "@Adds <COUNT> <NAME> to mech's inventory",
     btech_command_invoke_mech_raddstuff},

    {0, "FIXSTUFF", "@Fixes consistency errors in econ data",
     btech_command_invoke_mech_rfixstuff},
    {0, "CLEARSTUFF", "@Removes all stuff from 'mech",
     btech_command_invoke_mech_rresetstuff},

    {0, "REMOVESTUFF <NAME> <COUNT>",
     "@Removes <COUNT> <NAME> from mech's inventory",
     btech_command_invoke_mech_rremovestuff},
    {0, "SETMAPINDX <NUM>", "@Sets the mech's map index to num.",
     btech_command_invoke_mech_rsetmapindex},
    {0, "SETTEAM <NUM>", "@Sets the teams.",
     btech_command_invoke_mech_rsetteam},
    {0, "SETXY <X> <Y>", "@Sets the x & y value of the mech.",
     btech_command_invoke_mech_rsetxy},
    {0, NULL, NULL, NULL}};

size_t mech_command_count(void) {
  return (sizeof(MECHCOMMANDS) / sizeof(*MECHCOMMANDS)) - 1;
}
