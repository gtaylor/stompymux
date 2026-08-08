
/*
   p.autopilot_commands.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:35 CET 1999 from autopilot_commands.c */

#pragma once

#include "mux/server/platform.h"

typedef struct Autopilot Autopilot;
typedef struct AutopilotCommandDefinition AutopilotCommandDefinition;

const AutopilotCommandDefinition *autopilot_command_definition_at(int index);

/* autopilot_commands.c */
int auto_valid_progline(Autopilot *a, int p);
void auto_jump(DbRef player, void *data, char *buffer);
