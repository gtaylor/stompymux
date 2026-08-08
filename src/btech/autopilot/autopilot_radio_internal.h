#pragma once

#include "autopilot.h"

typedef void (*AutopilotRadioHandler)(Autopilot *autopilot, Mech *mech,
                                      AutopilotArgumentList *arguments,
                                      int argument_count, char *message);

typedef struct AutopilotRadioCommand {
  char const *abbreviation;
  char const *name;
  int argument_count;
  bool silent;
  AutopilotRadioHandler handler;
} AutopilotRadioCommand;

extern AutopilotRadioCommand const autopilot_radio_commands[];
const AutopilotRadioCommand *autopilot_radio_command_at(int index);

void autopilot_radio_clear_commands(Autopilot *autopilot, char *buffer);
