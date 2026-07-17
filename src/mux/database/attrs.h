/* attrs.h - Attribute definitions */

#include "mux/server/platform.h"

#pragma once

/* Attribute flags */
constexpr int AF_ODARK = 0x0001;  /* players other than owner can't see it */
constexpr int AF_DARK = 0x0002;   /* No one can see it */
constexpr int AF_WIZARD = 0x0004; /* only wizards can change it */
constexpr int AF_MDARK = 0x0008;  /* Only wizards can see it. Dark to mortals */
constexpr int AF_INTERNAL = 0x0010; /* Don't show even to #1 */
constexpr int AF_NOCMD = 0x0020;    /* Don't create a @ command for it */
constexpr int AF_LOCK = 0x0040;     /* Attribute is locked */
constexpr int AF_DELETED = 0x0080;  /* Attribute should be ignored */
constexpr int AF_NOPROG = 0x0100;  /* Don't process $-commands from this attr */
constexpr int AF_GOD = 0x0200;     /* Only #1 can change it */
constexpr int AF_IS_LOCK = 0x0400; /* Attribute is a lock */
constexpr int AF_VISUAL = 0x0800;  /* Anyone can see */
constexpr int AF_PRIVATE = 0x1000; /* Not inherited by children */
constexpr int AF_DIRTY = 0x2000;   /* This attribute has been compiled. */
constexpr int AF_REGEXP = 0x8000;  /* Do a regexp rather than wildcard match */

constexpr int A_OSUCC = 1; /* Others success message */
constexpr int A_OFAIL = 2; /* Others fail message */
constexpr int A_FAIL = 3;  /* Invoker fail message */
constexpr int A_SUCC = 4;  /* Invoker success message */
constexpr int A_PASS = 5;  /* Password (only meaningful for players) */
constexpr int A_DESC = 6;  /* Description */
/* 7 is reserved for the removed Sex attribute. */
constexpr int A_ODROP = 8; /* Others drop message */
constexpr int A_DROP = 9;  /* Invoker drop message */
/* 10 and 11 are reserved for the removed OKILL and KILL attributes. */
constexpr int A_ASUCC = 12; /* Success action list */
constexpr int A_AFAIL = 13; /* Failure action list */
constexpr int A_ADROP = 14; /* Drop action list */
/* 15 is reserved for the removed AKILL attribute. */
constexpr int A_AUSE = 16;    /* Use action list */
constexpr int A_CHARGES = 17; /* Number of charges remaining */
constexpr int A_RUNOUT = 18;  /* Actions done when no more charges */
constexpr int A_STARTUP = 19; /* Actions run when game started up */
constexpr int A_ACLONE = 20;  /* Actions run when obj is cloned */
/* 21 through 25 are reserved for removed payment and money attributes. */
constexpr int A_LISTEN = 26;   /* (Wildcarded) string to listen for */
constexpr int A_AAHEAR = 27;   /* Actions to do when anyone says LISTEN str */
constexpr int A_AMHEAR = 28;   /* Actions to do when I say LISTEN str */
constexpr int A_AHEAR = 29;    /* Actions to do when others say LISTEN str */
constexpr int A_LAST = 30;     /* Date/time of last login (players only) */
constexpr int A_QUEUEMAX = 31; /* Max. # of entries obj has in the queue */
constexpr int A_IDESC = 32;    /* Inside description (ENTER to get inside) */
constexpr int A_ENTER = 33;    /* Invoker enter message */
constexpr int A_OXENTER = 34;  /* Others enter message in dest */
constexpr int A_AENTER = 35;   /* Enter action list */
constexpr int A_ADESC = 36;    /* Describe action list */
constexpr int A_ODESC = 37;    /* Others describe message */
/* 38 is reserved for the removed building quota attribute. */
constexpr int A_ACONNECT = 39;    /* Actions run when player connects */
constexpr int A_ADISCONNECT = 40; /* Actions run when player disconnectes */
/* 41 is reserved for the removed money allowance attribute. */
constexpr int A_LOCK = 42;      /* Object lock */
constexpr int A_NAME = 43;      /* Object name */
constexpr int A_COMMENT = 44;   /* Wizard-accessable comments */
constexpr int A_USE = 45;       /* Invoker use message */
constexpr int A_OUSE = 46;      /* Others use message */
constexpr int A_SEMAPHORE = 47; /* Semaphore control info */
constexpr int A_TIMEOUT = 48;   /* Per-user disconnect timeout */
/* 49 is reserved for the removed building quota attribute. */
constexpr int A_LEAVE = 50;     /* Invoker leave message */
constexpr int A_OLEAVE = 51;    /* Others leave message in src */
constexpr int A_ALEAVE = 52;    /* Leave action list */
constexpr int A_OENTER = 53;    /* Others enter message in src */
constexpr int A_OXLEAVE = 54;   /* Others leave message in dest */
constexpr int A_MOVE = 55;      /* Invoker move message */
constexpr int A_OMOVE = 56;     /* Others move message */
constexpr int A_AMOVE = 57;     /* Move action list */
constexpr int A_ALIAS = 58;     /* Alias for player names */
constexpr int A_LENTER = 59;    /* ENTER lock */
constexpr int A_LLEAVE = 60;    /* LEAVE lock */
constexpr int A_LPAGE = 61;     /* PAGE lock */
constexpr int A_LUSE = 62;      /* USE lock */
constexpr int A_LGIVE = 63;     /* Give lock (who may give me away?) */
constexpr int A_EALIAS = 64;    /* Alternate names for ENTER */
constexpr int A_LALIAS = 65;    /* Alternate names for LEAVE */
constexpr int A_EFAIL = 66;     /* Invoker entry fail message */
constexpr int A_OEFAIL = 67;    /* Others entry fail message */
constexpr int A_AEFAIL = 68;    /* Entry fail action list */
constexpr int A_LFAIL = 69;     /* Invoker leave fail message */
constexpr int A_OLFAIL = 70;    /* Others leave fail message */
constexpr int A_ALFAIL = 71;    /* Leave fail action list */
constexpr int A_REJECT = 72;    /* Rejected page return message */
constexpr int A_AWAY = 73;      /* Not_connected page return message */
constexpr int A_IDLE = 74;      /* Success page return message */
constexpr int A_UFAIL = 75;     /* Invoker use fail message */
constexpr int A_OUFAIL = 76;    /* Others use fail message */
constexpr int A_AUFAIL = 77;    /* Use fail action list */
constexpr int A_PFAIL = 78;     /* Invoker page fail message */
constexpr int A_TPORT = 79;     /* Invoker teleport message */
constexpr int A_OTPORT = 80;    /* Others teleport message in src */
constexpr int A_OXTPORT = 81;   /* Others teleport message in dst */
constexpr int A_ATPORT = 82;    /* Teleport action list */
constexpr int A_PRIVS = 83;     /* Individual permissions */
constexpr int A_LOGINDATA = 84; /* Recent login information */
constexpr int A_LTPORT = 85;    /* Teleport lock (can others @tel to me?) */
constexpr int A_LDROP = 86;     /* Drop lock (can I be dropped or @tel'ed) */
constexpr int A_LRECEIVE = 87;  /* Receive lock (who may give me things?) */
constexpr int A_LASTSITE = 88;  /* Last site logged in from, in cleartext */
constexpr int A_INPREFIX = 89;  /* Prefix on incoming messages into objects */
constexpr int A_PREFIX = 90;    /* Prefix used by exits/objects when audible */
constexpr int A_INFILTER = 91;  /* Filter to zap incoming text into objects */
constexpr int A_FILTER = 92;    /* Filter to zap text forwarded by audible. */
constexpr int A_LLINK = 93;     /* Who may link to here */
constexpr int A_LTELOUT = 94;   /* Who may teleport out from here */
constexpr int A_FORWARDLIST = 95; /* Recipients of AUDIBLE output */
constexpr int A_LUSER = 97;       /* Spare lock not referenced by server */
constexpr int A_LPARENT = 98;     /* Legacy ParentLock attribute */
constexpr int A_VA = 100;         /* VA attribute (VB-VZ follow) */

constexpr int A_GFAIL = 129;      /* Give fail message */
constexpr int A_OGFAIL = 130;     /* Others give fail message */
constexpr int A_AGFAIL = 131;     /* Give fail action */
constexpr int A_RFAIL = 132;      /* Receive fail message */
constexpr int A_ORFAIL = 133;     /* Others receive fail message */
constexpr int A_ARFAIL = 134;     /* Receive fail action */
constexpr int A_DFAIL = 135;      /* Drop fail message */
constexpr int A_ODFAIL = 136;     /* Others drop fail message */
constexpr int A_ADFAIL = 137;     /* Drop fail action */
constexpr int A_TFAIL = 138;      /* Teleport (to) fail message */
constexpr int A_OTFAIL = 139;     /* Others teleport (to) fail message */
constexpr int A_ATFAIL = 140;     /* Teleport fail action */
constexpr int A_TOFAIL = 141;     /* Teleport (from) fail message */
constexpr int A_OTOFAIL = 142;    /* Others teleport (from) fail message */
constexpr int A_ATOFAIL = 143;    /* Teleport (from) fail action */
constexpr int A_LASTNAME = 144;   /* Last time you changed your name */
constexpr int A_LUAPARENT = 145;  /* Lua module attached to this object */
constexpr int A_MECHPREFID = 146; /* Preferred Mech ID on map */
constexpr int A_MAPCOLOR = 147;   /* ANSIMAP color scheme */

constexpr int A_LASTPAGE = 200;  /* Player last paged */
constexpr int A_DAILY = 204;     /* Daily attribute to be executed */
constexpr int A_LSPEECH = 209;   /* Speechlocks */
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
constexpr int A_MECHSTATUS = 222; /* Mech status string. Not to be tampered. */
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

constexpr int A_HOURLY = 246;
constexpr int A_AMECHDEST = 247;
constexpr int A_AMINETRIGGER = 248;
constexpr int A_AAEROLAND = 249;
constexpr int A_AOODLAND = 250;

/* #define FREE 251 Was A_HTDESC */

constexpr int A_VLIST = 252;
constexpr int A_LIST = 253;
constexpr int A_STRUCT = 254;
constexpr int A_TEMP = 255;

constexpr int A_USER_START = 256; /* Start of user-named attributes */
constexpr int ATR_BUF_CHUNK =
    100;                        /* Min size to allocate for attribute buffer */
constexpr int ATR_BUF_INCR = 6; /* Max size of one attribute */
