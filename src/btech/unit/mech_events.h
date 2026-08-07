
/*
 * $Id: mech.events.h,v 1.2 2005/08/03 21:40:54 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Created: Fri Aug 30 15:32:12 1996 fingon
 * Last modified: Sat Jun  6 20:14:55 1998 fingon
 *
 */

#pragma once

/* Semi-combat-related events */
constexpr int EVENT_MOVE = 1; /* mech */
/* Updates mech's position and the positionchanged flag */
constexpr int EVENT_DHIT = 2;    /* <artydata> */
constexpr int EVENT_STARTUP = 3; /* mech,timer */
/* Starts up da mech (timer = int which shows da stage of startup) */
constexpr int EVENT_LOCK = 4; /* mech,target */
/* Engages lock between <mech>,and <target> (breaking LOS stops this) */
constexpr int EVENT_STAND = 5; /* mech */
                               /* Makes da mech stand */
constexpr int EVENT_JUMP = 6;
/* Advances us one jump 'step' */
constexpr int EVENT_RECYCLE = 7;    /* CONVERTED. mech */
                                    /* Weapons recycling.. */
constexpr int EVENT_JUMPSTABIL = 8; /* mech */
/* If event of this type doesn't exist for mech, we've finished
   stabilizing after last jump */
constexpr int EVENT_RECOVERY = 9; /* mech */
/* Mech's pilot has chance of recovering from uncon */
constexpr int EVENT_SCHANGE =
    10; /* mech, <modes> (with first as higher bytes) */
        /* Sensor mode's changing.. */
constexpr int EVENT_DECORATION =
    11; /* timed decoration removal/happening thingy */
        /* map, mapobj */
constexpr int EVENT_SPOT_LOCK = 12;  /* spot-lock (Nim's stuff) */
constexpr int EVENT_PLOS = 13;       /* Possible-lock (mech) */
constexpr int EVENT_SPOT_CHECK = 14; /* Range-check for IDF */
constexpr int EVENT_TAKEOFF = 15;    /* Aero takeoff (mech, secstilllaunch) */
constexpr int EVENT_FALL = 16;       /* Shutdown mech falling */
constexpr int EVENT_BREGEN = 17;     /* Building regeneration */
constexpr int EVENT_BREBUILD = 18;   /* Building rebuild */
constexpr int EVENT_DUMP = 19;       /* mech, loc : Dump something */

constexpr int EVENT_MASC_FAIL = 20;  /* MASC roll for failure of stuff */
constexpr int EVENT_MASC_REGEN = 21; /* MASC recovery during non-use */
#define EVENT_AMMOWARN                                                         \
  22 /* CONVERTED. NO EVENT NEEDED NOW.  Warn of running out of ammo */

constexpr int FIRST_AUTO_EVENT = 23;
constexpr int EVENT_AUTOGOTO = 23;  /* Autopilot goto */
constexpr int EVENT_AUTOLEAVE = 24; /* Autopilot leavebase */
constexpr int EVENT_AUTOCOM = 25;   /* Autopilot next command */
constexpr int EVENT_AUTOGUN = 26;   /* Autopilot gun control */
constexpr int EVENT_AUTO_SENSOR =
    27;                              /* CONVERTED Autopilot gun/sensor change */
constexpr int EVENT_AUTOFOLLOW = 28; /* Autopilot follow */
constexpr int EVENT_AUTOENTERBASE = 29; /* Autopilot enterbase */
constexpr int EVENT_AUTO_REPLY = 30;    /* Autopilot reply */
constexpr int EVENT_AUTO_PROFILE = 31;  /* Autopilot profile change */
constexpr int EVENT_AUTO_ROAM = 32;
#define LAST_AUTO_EVENT EVENT_AUTO_ROAM

constexpr int EVENT_MRECOVERY = 33; /* mech */
constexpr int EVENT_BLINDREC = 34;
constexpr int EVENT_BURN = 35;
constexpr int EVENT_SS = 36;

constexpr int EVENT_HIDE = 37;
constexpr int EVENT_OOD = 38;
constexpr int EVENT_NUKEMECH = 39;
constexpr int EVENT_LATERAL = 40;
constexpr int EVENT_EXPLODE = 41;
constexpr int EVENT_DIG = 42;

constexpr int FIRST_TECH_EVENT = 43;

#define EVENT_REPAIR_REPL FIRST_TECH_EVENT         /* mech,<part> */
#define EVENT_REPAIR_REPLG (FIRST_TECH_EVENT + 1)  /* mech,<part> */
#define EVENT_REPAIR_REAT (FIRST_TECH_EVENT + 2)   /* mech,<location> */
#define EVENT_REPAIR_RELO (FIRST_TECH_EVENT + 3)   /* mech,<part/amount> */
#define EVENT_REPAIR_FIX (FIRST_TECH_EVENT + 4)    /* mech,<loc/amount/type> */
#define EVENT_REPAIR_FIXI (FIRST_TECH_EVENT + 5)   /* mech,<loc/amount/type> */
#define EVENT_REPAIR_SCRL (FIRST_TECH_EVENT + 6)   /* mech, loc */
#define EVENT_REPAIR_SCRP (FIRST_TECH_EVENT + 7)   /* mech, loc, part */
#define EVENT_REPAIR_SCRG (FIRST_TECH_EVENT + 8)   /* mech, loc, part */
#define EVENT_REPAIR_REPAG (FIRST_TECH_EVENT + 9)  /* mech,<part> */
#define EVENT_REPAIR_REPAP (FIRST_TECH_EVENT + 10) /* mech,<part> */
#define EVENT_REPAIR_MOB (FIRST_TECH_EVENT + 11)   /* mech,<part> */
#define EVENT_REPAIR_UMOB (FIRST_TECH_EVENT + 12)  /* mech,<part> */
#define EVENT_REPAIR_RESE (FIRST_TECH_EVENT + 13)  /* mech,<location> */
#define EVENT_REPAIR_REPSUIT (FIRST_TECH_EVENT + 14)    /* mech */
#define EVENT_REPAIR_REPENHCRIT (FIRST_TECH_EVENT + 15) /* mech */

#define LAST_TECH_EVENT EVENT_REPAIR_REPENHCRIT

constexpr int EVENT_STANDFAIL = 60;
constexpr int EVENT_SLITECHANGING = 61;
constexpr int EVENT_HEATCUTOFFCHANGING = 62;
constexpr int EVENT_VEHICLEBURN = 63; /* Burn a side of a vehicle */
constexpr int EVENT_UNSTUN_CREW = 64; /* Unstun the crew */
constexpr int EVENT_CREWSTUN = 65;
constexpr int EVENT_UNJAM_TURRET = 66;
constexpr int EVENT_UNJAM_AMMO = 67;
constexpr int EVENT_STEALTH_ARMOR = 68;
constexpr int EVENT_NSS = 69;
constexpr int EVENT_TAG_RECYCLE = 70;
constexpr int EVENT_REMOVE_PODS = 71;
constexpr int EVENT_VEHICLE_EXTINGUISH = 72;
constexpr int EVENT_ENTER_HANGAR = 73;
constexpr int EVENT_CHANGING_HULLDOWN = 74;

/* Not used in the stable branch, just devel */
/* EVENT_BOGDOWNWAIT                75 */

constexpr int EVENT_SCHARGE_FAIL = 76;  /* SCHARGE roll for failure of stuff */
constexpr int EVENT_SCHARGE_REGEN = 77; /* SCHARGE recovery during non-use */

constexpr int EVENT_CHECK_STAGGER = 78;
constexpr int EVENT_MOVEMODE = 79;
constexpr int EVENT_SIDESLIP = 80;

#define ETEMPL(a) void a(MuxEvent *e)
