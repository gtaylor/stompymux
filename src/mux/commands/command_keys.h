/** @file
 * Shared native command invocation keys.
 */
#pragma once

constexpr int BOOT_QUIET = 1; /* Inhibit boot message to victim. */
constexpr int BOOT_PORT = 2;  /* Boot by port number. */

constexpr int BTECH_INFO = 0;
constexpr int BTECH_REGISTER = 1;
constexpr int BTECH_UNREGISTER = 2;

typedef enum MechAdminCommandKey : int {
  MECH_ADMIN_LOADNEW = 1,
  MECH_ADMIN_RESTORE,
  MECH_ADMIN_SAVENEW,
  MECH_ADMIN_SETARMOR,
  MECH_ADMIN_ADDWEAP,
  MECH_ADMIN_RESETCRITS,
  MECH_ADMIN_REPAIR,
  MECH_ADMIN_RELOAD,
  MECH_ADMIN_RESTOCK,
  MECH_ADMIN_FIREMODE,
  MECH_ADMIN_ADDSP,
  MECH_ADMIN_DISPLAY,
  MECH_ADMIN_SHOWTECH,
  MECH_ADMIN_ADDTECH,
  MECH_ADMIN_DELTECH,
  MECH_ADMIN_ADDINFTECH,
  MECH_ADMIN_DELINFTECH,
  MECH_ADMIN_SETTONS,
  MECH_ADMIN_SETTYPE,
  MECH_ADMIN_SETMOVE,
  MECH_ADMIN_SETMAXSPEED,
  MECH_ADMIN_SETHEATSINKS,
  MECH_ADMIN_SETJUMPSPEED,
  MECH_ADMIN_SETLRSRANGE,
  MECH_ADMIN_SETTACRANGE,
  MECH_ADMIN_SETSCANRANGE,
  MECH_ADMIN_SETRADIO,
  MECH_ADMIN_SETRADIORANGE,
  MECH_ADMIN_SETCARGOSPACE,
} MechAdminCommandKey;

constexpr int CLONE_LOCATION = 0;  /* Create clone in current location. */
constexpr int CLONE_INVENTORY = 4; /* Create clone in current inventory. */

constexpr int DEST_ONE = 1;       /* Destroy one object. */
constexpr int DEST_OVERRIDE = 4;  /* Override safety check. */
constexpr int DEST_RECURSIVE = 8; /* Destroy recursively. */

constexpr int DESCRIPTION_EXTERNAL = 0;
constexpr int DESCRIPTION_INTERNAL = 1;

constexpr int DIG_TELEPORT = 1; /* Teleport after digging. */
constexpr int EXAM_BRIEF = 1;   /* Omit ordinary attribute list. */
constexpr int EXAM_DEBUG = 4;   /* Display database debugging details. */
constexpr int FRC_COMMAND = 1;  /* Force command form. */
constexpr int GIVE_QUIET = 64;  /* Inhibit give messages. */
constexpr int GLOB_ENABLE = 1;  /* Enable a global command. */
constexpr int GLOB_DISABLE = 2; /* Disable a global command. */
constexpr int HALT_ALL = 1;     /* Halt every queued command. */
constexpr int OPEN_LOCATION = 0;
constexpr int OPEN_INVENTORY = 1;
constexpr int PASS_ANY = 1;    /* New-password form. */
constexpr int PCRE_PLAYER = 1; /* Create player. */
constexpr int SET_QUIET = 1;   /* Suppress "Set." message. */
constexpr int SRCH_SEARCH = 1; /* Perform a normal search. */
constexpr int TELEPORT_DEFAULT = 1;
constexpr int TELEPORT_QUIET = 2;
