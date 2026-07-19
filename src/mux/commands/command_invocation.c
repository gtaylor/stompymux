/* command_invocation.c - Typed legacy command adapter calls. */

#include "mux/commands/command_invocation.h"

void command_invocation_call_no_arguments(CommandNoArgumentsHandler handler,
                                          CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key);
}

void command_invocation_call_unparsed(CommandUnparsedHandler handler,
                                      CommandInvocation *invocation) {
  handler(invocation->player, invocation->unparsed);
}

void command_invocation_call_one_argument(CommandOneArgumentHandler handler,
                                          CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key,
          invocation->first);
}

void command_invocation_call_vector(CommandVectorHandler handler,
                                    CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key,
          invocation->first, invocation->vector, invocation->vector_count);
}

void command_invocation_call_two_arguments(CommandTwoArgumentsHandler handler,
                                           CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key,
          invocation->first, invocation->second);
}

void command_invocation_call_two_arguments_vector(
    CommandTwoArgumentsVectorHandler handler, CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key,
          invocation->first, invocation->second, invocation->vector,
          invocation->vector_count);
}

void command_invocation_call_two_vectors(CommandTwoVectorsHandler handler,
                                         CommandInvocation *invocation) {
  handler(invocation->player, invocation->cause, invocation->key,
          invocation->first, invocation->vector, invocation->vector_count,
          invocation->command_arguments, invocation->command_argument_count);
}
