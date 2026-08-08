/* Private command registry definitions shared by command implementations. */

#pragma once

#include <stddef.h>

#include "mux/commands/command.h"

enum : int {
  SW_MULTIPLE = (int)0x80000000U,
  SW_GOT_UNIQUE = 0x40000000,
};

extern CMDENT command_table[];
size_t command_table_entry_count(void);
CMDENT *command_table_entry_at(size_t index);
CMDENT *command_prefix_entry_at(const CommandRegistry *registry, size_t index);
void command_prefix_entry_set(CommandRegistry *registry, size_t index,
                              CMDENT *entry);

void command_list_access(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration,
                         CommandRegistry *registry, DbRef player);
void command_list_switches(EvaluationContext *evaluation,
                           const ServerConfiguration *configuration,
                           DbRef player);
