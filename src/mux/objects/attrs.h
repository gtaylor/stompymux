/* attrs.h - Attribute definitions */

#include "mux/server/platform.h"

#pragma once

/* 1 through 4 are reserved for removed action and lock-failure attributes. */
constexpr int A_PASS = 5; /* Password (only meaningful for players) */
constexpr int A_DESC = 6; /* Description */
/* 7 is reserved for the removed Sex attribute. */
/* 8 and 9 are reserved for removed action-message attributes. */
/* 10 and 11 are reserved for the removed OKILL and KILL attributes. */
/* 12 through 20 are reserved for removed action attributes. */
/* 21 through 25 are reserved for removed payment and money attributes. */
/* 26 through 29 are reserved for removed listen attributes and actions. */
constexpr int A_LAST = 30;     /* Date/time of last login (players only) */
constexpr int A_QUEUEMAX = 31; /* Max. # of entries obj has in the queue */
constexpr int A_IDESC = 32;    /* Inside description (ENTER to get inside) */
/* 33 and 34 are reserved for removed action-message attributes. */
/* 35 and 36 are reserved for removed action attributes. */
/* 37 is reserved for the removed Odesc attribute. */
/* 38 is reserved for the removed building quota attribute. */
/* 39 and 40 are reserved for removed connection action attributes. */
/* 41 is reserved for the removed money allowance attribute. */
/* 42 is reserved for the removed DefaultLock attribute. */
constexpr int A_NAME = 43; /* Object name */
/* 44 is reserved for the removed Comment attribute. */
/* 45 and 46 are reserved for removed action-message attributes. */
/* 47 is reserved. */
constexpr int A_TIMEOUT = 48; /* Per-user disconnect timeout */
/* 49 is reserved for the removed building quota attribute. */
/* 50 and 51 are reserved for removed action-message attributes. */
/* 52 is reserved for the removed Aleave attribute. */
/* 53 through 56 are reserved for removed action-message attributes. */
/* 57 is reserved for the removed Amove attribute. */
constexpr int A_ALIAS = 58; /* Alias for player names */
/* 59 and 60 are reserved for removed lock attributes. */
/* 61 is reserved for the removed PAGE lock. */
/* 62 and 63 are reserved for removed lock attributes. */
/* 64 and 65 are reserved for the removed enter and leave aliases. */
/* 66 and 67 are reserved for removed lock failure attributes. */
/* 68 is reserved for the removed Aefail attribute. */
/* 69 and 70 are reserved for removed lock failure attributes. */
/* 71 is reserved for the removed Alfail attribute. */
constexpr int A_REJECT = 72; /* Rejected page return message */
constexpr int A_AWAY = 73;   /* Not_connected page return message */
constexpr int A_IDLE = 74;   /* Success page return message */
/* 75 and 76 are reserved for removed lock failure attributes. */
/* 77 is reserved for the removed Aufail attribute. */
constexpr int A_PFAIL = 78; /* Invoker page fail message */
/* 79 through 81 are reserved for removed action-message attributes. */
/* 82 is reserved for the removed Atport attribute. */
constexpr int A_PRIVS = 83;     /* Individual permissions */
constexpr int A_LOGINDATA = 84; /* Recent login information */
/* 85 through 87 are reserved for removed lock attributes. */
constexpr int A_LASTSITE = 88; /* Last site logged in from, in cleartext */
/* 89 through 92 are reserved for removed message prefix/filter attributes. */
/* 93 and 94 are reserved for removed lock attributes. */
/* 95 is reserved for the removed forwarding-list attribute. */
/* 97 and 98 are reserved for removed lock attributes. */

/* 129 and 130 are reserved for removed lock failure attributes. */
/* 131 is reserved for the removed Agfail attribute. */
/* 132 and 133 are reserved for removed lock failure attributes. */
/* 134 is reserved for the removed Arfail attribute. */
/* 135 and 136 are reserved for removed lock failure attributes. */
/* 137 is reserved for the removed Adfail attribute. */
/* 138 and 139 are reserved for removed lock failure attributes. */
/* 140 is reserved for the removed Atfail attribute. */
/* 141 and 142 are reserved for removed lock failure attributes. */
/* 143 is reserved for the removed Atofail attribute. */
constexpr int A_LASTNAME = 144; /* Last time you changed your name */
/* 145 is reserved for the Lua parent field, which is stored on the object. */
constexpr int A_MECHPREFID = 146; /* Preferred Mech ID on map */
constexpr int A_MAPCOLOR = 147;   /* ANSIMAP color scheme */

constexpr int A_LASTPAGE = 200; /* Player last paged */
/* 204 is reserved for the removed Daily attribute. */
/* 209 is reserved for the removed SpeechLock attribute. */
constexpr int A_DESTROYER = 212; /* Who is destroying this object? */
constexpr int A_UNUSED1 = 213;   /* Old luck.c, now unused. */

/* Mecha stuff */
constexpr int A_MECHSKILLS = 214; /* Pilot's skills in using a mech */
constexpr int A_XTYPE = 215;      /* Hardcode type */
constexpr int A_TACSIZE = 216;    /* Tactical Size (H & W) */
constexpr int A_LRSHEIGHT = 217;  /* LRS height */
constexpr int A_CONTACTOPT = 218; /* Contact options */
constexpr int A_MECHNAME = 219;   /* Mech name */
constexpr int A_MECHTYPE = 220;   /* Mech type */
constexpr int A_MECHDESC = 221;   /* Mech extra desc (for view) */
/* Attribute number 222 is reserved after removal of softcoded mech status. */
constexpr int A_MWTEMPLATE = 229; /* MW template to use (if any) */
constexpr int A_FACTION = 230;    /* Faction */
constexpr int A_JOB = 231;        /* Job field */
constexpr int A_RANKNUM =
    232; /* 'true' rank, the thing comparisons are done with */

/* BT-stats: */
constexpr int A_HEALTH = 233; /* Bruise,Lethal */
constexpr int A_ATTRS = 234;  /* Attributes */

constexpr int A_BUILDLINKS = 235;    /* Links */
constexpr int A_BUILDENTRANCE = 236; /* Entrance(s) */
constexpr int A_BUILDCOORD = 237;    /* X/Y coord */

/* BT-stats: */
constexpr int A_ADVS = 238;     /* Advantages */
constexpr int A_PILOTNUM = 239; /* Mech's pilot # */
constexpr int A_MAPVIS = 240;   /* Visibility */
constexpr int A_TZ = 241;       /* Timezone */
constexpr int A_TECHTIME = 242; /* Time (as a time_t number) until completion */
constexpr int A_ECONPARTS = 243; /* Econ parts */

/* BT-stats: */
constexpr int A_SKILLS = 244;  /* Skills */
constexpr int A_PCEQUIP = 245; /* PCombat equipment */

/* 246 through 250 are reserved for removed action attributes. */

/* #define FREE 251 Was A_HTDESC */

constexpr int A_VLIST = 252;
constexpr int A_LIST = 253;
constexpr int A_STRUCT = 254;
constexpr int A_TEMP = 255;

constexpr int A_USER_START = 256; /* Start of user-named attributes */
constexpr int ATR_BUF_CHUNK =
    100;                        /* Min size to allocate for attribute buffer */
constexpr int ATR_BUF_INCR = 6; /* Max size of one attribute */
