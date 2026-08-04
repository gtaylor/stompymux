/* Private command registry definitions shared by command implementations. */

#pragma once

#include "mux/commands/command.h"

enum : int {
  SW_MULTIPLE = (int)0x80000000U,
  SW_GOT_UNIQUE = 0x40000000,
};

extern CMDENT command_table[];

void command_list_access(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration,
                         CommandRegistry *registry, DbRef player);
void command_list_switches(EvaluationContext *evaluation,
                           const ServerConfiguration *configuration,
                           DbRef player);
