
/* Declares scheduled event interfaces for units. */

#pragma once

/* Semi-combat-related events. Values are persisted dispatch identifiers. */
typedef enum MechEventType : int {
  EVENT_MOVE = 1,    /* mech */
                     /* Updates mech's position and the positionchanged flag */
  EVENT_DHIT = 2,    /* <artydata> */
  EVENT_STARTUP = 3, /* mech,timer */
  /* Starts up da mech (timer = int which shows da stage of startup) */
  EVENT_LOCK = 4, /* mech,target */
  /* Engages lock between <mech>,and <target> (breaking LOS stops this) */
  EVENT_STAND = 5, /* mech */
                   /* Makes da mech stand */
  EVENT_JUMP = 6,
  /* Advances us one jump 'step' */
  EVENT_RECYCLE = 7,    /* CONVERTED. mech */
                        /* Weapons recycling.. */
  EVENT_JUMPSTABIL = 8, /* mech */
  /* If event of this type doesn't exist for mech, we've finished
     stabilizing after last jump */
  EVENT_RECOVERY = 9,    /* mech */
                         /* Mech's pilot has chance of recovering from uncon */
  EVENT_SCHANGE = 10,    /* mech, <modes> (with first as higher bytes) */
                         /* Sensor mode's changing.. */
  EVENT_DECORATION = 11, /* timed decoration removal/happening thingy */
                         /* map, mapobj */
  EVENT_SPOT_LOCK = 12,  /* spot-lock (Nim's stuff) */
  EVENT_PLOS = 13,       /* Possible-lock (mech) */
  EVENT_SPOT_CHECK = 14, /* Range-check for IDF */
  EVENT_TAKEOFF = 15,    /* Aero takeoff (mech, secstilllaunch) */
  EVENT_FALL = 16,       /* Shutdown mech falling */
  EVENT_BREGEN = 17,     /* Building regeneration */
  EVENT_BREBUILD = 18,   /* Building rebuild */
  EVENT_DUMP = 19,       /* mech, loc : Dump something */
  EVENT_MASC_FAIL = 20,  /* MASC roll for failure of stuff */
  EVENT_MASC_REGEN = 21, /* MASC recovery during non-use */
  EVENT_AMMOWARN = 22,   /* Converted; no event needed now */
  FIRST_AUTO_EVENT = 23,
  EVENT_AUTOGOTO = 23,      /* Autopilot goto */
  EVENT_AUTOLEAVE = 24,     /* Autopilot leavebase */
  EVENT_AUTOCOM = 25,       /* Autopilot next command */
  EVENT_AUTOGUN = 26,       /* Autopilot gun control */
  EVENT_AUTO_SENSOR = 27,   /* Converted autopilot gun/sensor change */
  EVENT_AUTOFOLLOW = 28,    /* Autopilot follow */
  EVENT_AUTOENTERBASE = 29, /* Autopilot enterbase */
  EVENT_AUTO_REPLY = 30,    /* Autopilot reply */
  EVENT_AUTO_PROFILE = 31,  /* Autopilot profile change */
  EVENT_AUTO_ROAM = 32,
  LAST_AUTO_EVENT = 32,
  EVENT_MRECOVERY = 33, /* mech */
  EVENT_BLINDREC = 34,
  EVENT_BURN = 35,
  EVENT_SS = 36,
  EVENT_HIDE = 37,
  EVENT_OOD = 38,
  EVENT_NUKEMECH = 39,
  EVENT_LATERAL = 40,
  EVENT_EXPLODE = 41,
  EVENT_DIG = 42,
  FIRST_TECH_EVENT = 43,
  EVENT_REPAIR_REPL = 43,       /* mech,<part> */
  EVENT_REPAIR_REPLG = 44,      /* mech,<part> */
  EVENT_REPAIR_REAT = 45,       /* mech,<location> */
  EVENT_REPAIR_RELO = 46,       /* mech,<part/amount> */
  EVENT_REPAIR_FIX = 47,        /* mech,<loc/amount/type> */
  EVENT_REPAIR_FIXI = 48,       /* mech,<loc/amount/type> */
  EVENT_REPAIR_SCRL = 49,       /* mech, loc */
  EVENT_REPAIR_SCRP = 50,       /* mech, loc, part */
  EVENT_REPAIR_SCRG = 51,       /* mech, loc, part */
  EVENT_REPAIR_REPAG = 52,      /* mech,<part> */
  EVENT_REPAIR_REPAP = 53,      /* mech,<part> */
  EVENT_REPAIR_MOB = 54,        /* mech,<part> */
  EVENT_REPAIR_UMOB = 55,       /* mech,<part> */
  EVENT_REPAIR_RESE = 56,       /* mech,<location> */
  EVENT_REPAIR_REPSUIT = 57,    /* mech */
  EVENT_REPAIR_REPENHCRIT = 58, /* mech */
  LAST_TECH_EVENT = 58,
  EVENT_STANDFAIL = 60,
  EVENT_SLITECHANGING = 61,
  EVENT_HEATCUTOFFCHANGING = 62,
  EVENT_VEHICLEBURN = 63, /* Burn a side of a vehicle */
  EVENT_UNSTUN_CREW = 64, /* Unstun the crew */
  EVENT_CREWSTUN = 65,
  EVENT_UNJAM_TURRET = 66,
  EVENT_UNJAM_AMMO = 67,
  EVENT_STEALTH_ARMOR = 68,
  EVENT_NSS = 69,
  EVENT_TAG_RECYCLE = 70,
  EVENT_REMOVE_PODS = 71,
  EVENT_VEHICLE_EXTINGUISH = 72,
  EVENT_ENTER_HANGAR = 73,
  EVENT_CHANGING_HULLDOWN = 74,

  /* Not used in the stable branch, just devel */
  /* EVENT_BOGDOWNWAIT                75 */

  EVENT_SCHARGE_FAIL = 76,  /* SCHARGE roll for failure of stuff */
  EVENT_SCHARGE_REGEN = 77, /* SCHARGE recovery during non-use */
  EVENT_CHECK_STAGGER = 78,
  EVENT_MOVEMODE = 79,
  EVENT_SIDESLIP = 80,
} MechEventType;

static_assert(EVENT_MOVE == 1 && EVENT_AMMOWARN == 22);
static_assert(FIRST_AUTO_EVENT == 23 && LAST_AUTO_EVENT == 32);
static_assert(FIRST_TECH_EVENT == 43 && LAST_TECH_EVENT == 58);
static_assert(EVENT_CHANGING_HULLDOWN == 74 && EVENT_SCHARGE_FAIL == 76);
static_assert(EVENT_SIDESLIP == 80);
