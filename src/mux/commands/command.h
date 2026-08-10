/* command.h - Command parsing, dispatch, and command-handler declarations. */

#include "mux/server/platform.h"

#pragma once

#include "mux/commands/command_invocation.h"
#include "mux/commands/command_keys.h"

#include "mux/objects/db.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/support/name_table.h"

typedef struct CommandContext CommandContext;
typedef struct CommandRegistry CommandRegistry;
typedef struct ConfigurationContext ConfigurationContext;
typedef struct HashTable HashTable;
typedef struct ServerConfiguration ServerConfiguration;

typedef enum {
  LUA_COMMAND_CHECK = 1 << 0,
  LUA_COMMAND_PARENT = 1 << 1,
  LUA_COMMAND_RELOAD = 1 << 2,
  LUA_COMMAND_SCHEDULE = 1 << 3,
  LUA_COMMAND_VIEWPARENT = 1 << 4,
} LuaCommandKey;

typedef enum {
  STATE_COMMAND_EXAMINE = 1 << 0,
  STATE_COMMAND_SET = 1 << 1,
  STATE_COMMAND_WIPE = 1 << 2,
  STATE_COMMAND_COPY = 1 << 3,
  STATE_COMMAND_MOVE = 1 << 4,
} StateCommandKey;

int check_access(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef player,
                 int mask);
void set_prefix_cmds(CommandRegistry *registry);

/*
 * Commands are dispatched through the uniform typed invocation boundary.
 */
typedef union cmdentry_handler {
  CommandInvocationHandler invoke;
} CmdHandler;

typedef struct cmdentry CMDENT;
struct cmdentry {
  const char *cmdname;
  NameTable *switches;
  int perms;
  int extra;
  int callseq;
  CmdHandler handler;
};

void init_cmdtab(CommandRegistry *registry);
void command_aliases_destroy(HashTable *commands);
extern NameTable access_nametab[];
int cf_access(const ConfigurationCall *call);
int cf_cmd_alias(const ConfigurationCall *call);

/* Command handler call conventions */

constexpr int CS_NO_ARGS = 0x0000;   /* No arguments */
constexpr int CS_ONE_ARG = 0x0001;   /* One argument */
constexpr int CS_TWO_ARG = 0x0002;   /* Two arguments */
constexpr int CS_NARG_MASK = 0x0003; /* Argument count mask */
constexpr int CS_ARGV = 0x0004;      /* ARG2 is in ARGV form */
/* 0x0010 and 0x0020 are reserved after removal of command evaluation. */
constexpr int CS_CAUSE = 0x0040;   /* Pass cause to old command handler */
constexpr int CS_UNPARSE = 0x0080; /* Pass unparsed cmd to old-style handler */
/* 0x0100 and 0x0200 are reserved after removal of evaluator arguments. */
constexpr int CS_STRIP_AROUND =
    0x0400; /* Strip braces around entire string only */
/* 0x0800 is reserved for the removed softcode-added command convention. */
constexpr int CS_NO_MACRO = 0x1000; /* Command can't be used inside macro */
constexpr int CS_LEADIN = 0x2000;   /* Command is a single-letter lead-in */

/* Command permission flags */

constexpr int CA_PUBLIC = 0x00000000; /* No access restrictions */
constexpr int CA_GOD = 0x00000001;    /* GOD only... */
constexpr int CA_WIZARD = 0x00000002; /* Wizards only */
/* 0x00000004 is reserved for the removed builder power restriction. */
/* 0x00000008 is reserved for the removed immortal restriction. */
/* 0x00000010 is reserved for the removed robot-only restriction. */
/* 0x00000020 is reserved for the removed announce power restriction. */
constexpr int CA_ADMIN = 0x00000800; /* Wizard */
/* 0x00001000 is reserved for the removed no_haven restriction. */
/* 0x00002000 is reserved for the removed no-robot restriction. */
/* 0x00004000 is reserved for the removed no_slave restriction. */
constexpr int CA_NO_SUSPECT = 0x00008000; /* Not by SUSPECT players */
constexpr int CA_NO_IC = 0x00020000;      /* Not by IC players */

/* 0x01000000 is reserved for the removed global building restriction. */
constexpr int CA_QUEUE = 0x02000000;    /* Requires command queueing */
constexpr int CA_DISABLED = 0x04000000; /* Command completely disabled */
constexpr int CA_LOCATION = 0x10000000; /* Invoker must have location */
constexpr int CA_CONTENTS = 0x20000000; /* Invoker must have contents */
constexpr int CA_PLAYER = 0x40000000;   /* Invoker must be a player */
// Stored as int (not unsigned) so it ORs cleanly into CMDENT.perms without
// signedness conversions; C23 guarantees the top-bit pattern converts to
// INT_MIN deterministically.
constexpr int CF_DARK = (int)0x80000000U; /* Command doesn't show up in list */

void process_command(CommandContext *context, char *command, char *arguments[],
                     int argument_count);
