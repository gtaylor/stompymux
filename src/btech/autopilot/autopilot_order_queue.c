/* Implements safe queued autopilot order ownership. */

#include "autopilot_order_queue_api.h"

#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_argument_list_api.h"
#include "mux/support/doubly_linked_list.h"

bool autopilot_order_is_supported(int command_enum) {
  switch (command_enum) {
  case GOAL_CHASETARGET:
  case GOAL_DUMBFOLLOW:
  case GOAL_DUMBGOTO:
  case GOAL_ENTERBASE:
  case GOAL_FOLLOW:
  case GOAL_GOTO:
  case GOAL_LEAVEBASE:
  case GOAL_OLDGOTO:
  case GOAL_ROAM:
  case COMMAND_AUTOGUN:
  case COMMAND_DROPOFF:
  case COMMAND_EMBARK:
  case COMMAND_PICKUP:
  case COMMAND_SHUTDOWN:
  case COMMAND_SPEED:
  case COMMAND_STARTUP:
  case COMMAND_UDISEMBARK:
    return true;
  default:
    return false;
  }
}

size_t autopilot_order_count(const Autopilot *autopilot) {
  if (autopilot == nullptr || autopilot->commands == nullptr)
    return 0;
  const int count = doubly_linked_list_size(autopilot->commands);
  return count > 0 ? (size_t)count : 0;
}

const AutopilotCommand *autopilot_order_at(const Autopilot *autopilot,
                                           size_t index) {
  if (index >= autopilot_order_count(autopilot))
    return nullptr;
  return doubly_linked_list_get_node(autopilot->commands, (int)index + 1);
}

void auto_destroy_command_node(AutopilotCommand *node) {
  if (node == nullptr)
    return;
  autopilot_argument_list_destroy(&node->arguments);
  free(node);
}

AutopilotOrderResult
autopilot_order_enqueue(Autopilot *autopilot,
                        const AutopilotCommandDefinition *definition,
                        const AutopilotArgumentList *arguments) {
  if (autopilot == nullptr || autopilot->commands == nullptr ||
      definition == nullptr || definition->name == nullptr ||
      arguments == nullptr)
    return AUTOPILOT_ORDER_INVALID;
  if (!autopilot_order_is_supported(definition->command_enum))
    return AUTOPILOT_ORDER_UNSUPPORTED;
  if (autopilot_order_count(autopilot) >= AUTOPILOT_MEMORY)
    return AUTOPILOT_ORDER_FULL;
  if (definition->argcount < 0 || definition->argcount >= AUTOPILOT_MAX_ARGS ||
      arguments->capacity != AUTOPILOT_MAX_ARGS)
    return AUTOPILOT_ORDER_INVALID;
  for (size_t index = 0; index < AUTOPILOT_MAX_ARGS; index++) {
    const bool expected = index <= (size_t)definition->argcount;
    if ((autopilot_argument_list_get(arguments, index) != nullptr) != expected)
      return AUTOPILOT_ORDER_INVALID;
  }

  AutopilotCommand *command = calloc(1, sizeof(*command));
  if (command == nullptr)
    return AUTOPILOT_ORDER_NO_MEMORY;
  autopilot_argument_list_initialize(&command->arguments, AUTOPILOT_MAX_ARGS);
  for (size_t index = 0; index < AUTOPILOT_MAX_ARGS; index++) {
    const char *argument = autopilot_argument_list_get(arguments, index);
    if (argument == nullptr)
      continue;
    char *copy = strdup(argument);
    if (copy == nullptr) {
      auto_destroy_command_node(command);
      return AUTOPILOT_ORDER_NO_MEMORY;
    }
    autopilot_argument_list_set(&command->arguments, index, copy);
  }
  command->argcount = (unsigned char)definition->argcount;
  command->command_enum = definition->command_enum;
  command->ai_command_function = definition->ai_command_function;
  DoublyLinkedListNode *node = doubly_linked_list_create_node(command);
  if (node == nullptr) {
    auto_destroy_command_node(command);
    return AUTOPILOT_ORDER_NO_MEMORY;
  }
  doubly_linked_list_insert_end(autopilot->commands, node);
  return AUTOPILOT_ORDER_OK;
}

AutopilotOrderResult autopilot_order_remove(Autopilot *autopilot,
                                            size_t index) {
  if (autopilot == nullptr || index >= autopilot_order_count(autopilot))
    return AUTOPILOT_ORDER_NOT_FOUND;
  AutopilotCommand *command = doubly_linked_list_remove_node_at_pos(
      autopilot->commands, (int)index + 1);
  if (command == nullptr)
    return AUTOPILOT_ORDER_NOT_FOUND;
  auto_destroy_command_node(command);
  return AUTOPILOT_ORDER_OK;
}

void autopilot_order_clear(Autopilot *autopilot) {
  while (autopilot_order_count(autopilot) > 0)
    (void)autopilot_order_remove(autopilot, 0);
}

AutopilotOrderResult autopilot_order_pop(Autopilot *autopilot) {
  return autopilot_order_remove(autopilot, 0);
}
