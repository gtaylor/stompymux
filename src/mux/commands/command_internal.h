/* Private command registry definitions shared by command implementations. */

#pragma once

#include <stddef.h>

#include "mux/commands/command.h"
#include "mux/support/name_table.h"

extern const NameTable ACCESS_NAMETAB[];

enum : int {
  SW_MULTIPLE = (int)0x80000000U,
  SW_GOT_UNIQUE = 0x40000000,
};

CMDENT *command_prefix_entry_at(const CommandRegistry *registry, size_t index);

void command_list_access(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration,
                         const CommandRegistry *registry, DbRef player);
void command_list_switches(EvaluationContext *evaluation,
                           const ServerConfiguration *configuration,
                           const CommandRegistry *registry, DbRef player);
