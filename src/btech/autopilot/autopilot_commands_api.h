/* Declares the BattleTech autopilot commands API. */

#pragma once

#include "mux/server/platform.h"

typedef struct Autopilot Autopilot;
typedef struct AutopilotCommandDefinition AutopilotCommandDefinition;

const AutopilotCommandDefinition *autopilot_command_definition_at(int index);

/* autopilot_commands.c */
bool auto_valid_progline(Autopilot *a, int p);
void auto_jump(DbRef player, void *data, char *buffer);
