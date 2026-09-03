/** @file
 * Attribute definitions.
 */

#pragma once

constexpr int A_DESCRIPTION = 6; /* Description */
constexpr int A_INTERNAL_DESCRIPTION =
    32;                      /* Internal description (ENTER to get inside) */
constexpr int A_NAME = 43;   /* Object name */
constexpr int A_ALIAS = 58;  /* Alias for player names */
constexpr int A_REJECT = 72; /* Rejected page return message */
constexpr int A_AWAY = 73;   /* Not_connected page return message */
constexpr int A_IDLE = 74;   /* Success page return message */
constexpr int A_PFAIL = 78;  /* Invoker page fail message */
constexpr int A_MECHPREFID = 146; /* Preferred Mech ID on map */
constexpr int A_DESTROYER = 212;  /* Who is destroying this object? */

/* Mecha stuff */
constexpr int A_MECHSKILLS = 214;    /* Pilot's skills in using a mech */
constexpr int A_XTYPE = 215;         /* Hardcode type */
constexpr int A_TACSIZE = 216;       /* Tactical Size (H & W) */
constexpr int A_LRSHEIGHT = 217;     /* LRS height */
constexpr int A_CONTACTOPT = 218;    /* Contact options */
constexpr int A_MECHNAME = 219;      /* Mech name */
constexpr int A_MECHTYPE = 220;      /* Mech type */
constexpr int A_MECHDESC = 221;      /* Mech extra desc (for view) */
constexpr int A_MWTEMPLATE = 229;    /* MW template to use (if any) */
constexpr int A_BUILDLINKS = 235;    /* Links */
constexpr int A_BUILDENTRANCE = 236; /* Entrance(s) */
constexpr int A_BUILDCOORD = 237;    /* X/Y coord */
constexpr int A_PILOTNUM = 239;      /* Mech's pilot # */
constexpr int A_MAPVIS = 240;        /* Visibility */
constexpr int A_TECHTIME = 242; /* Time (as a time_t number) until completion */
constexpr int A_PCEQUIP = 245;  /* PCombat equipment */

constexpr int A_VLIST = 252;
constexpr int A_LIST = 253;
constexpr int A_STRUCT = 254;
constexpr int A_TEMP = 255;
