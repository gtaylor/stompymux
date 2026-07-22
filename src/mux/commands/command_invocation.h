/* command_invocation.h - Typed boundary between parsing and command handlers.
 */

#pragma once

#include "mux/objects/db.h"

typedef struct CommandContext CommandContext;
typedef struct CommandInvocation CommandInvocation;

struct CommandInvocation {
  /* Parsed fields and their command context are borrowed for one dispatch. */
  CommandContext *context;
  DbRef player;
  DbRef cause;
  int key;
  char *unparsed;
  char *first;
  char *second;
  char **vector;
  int vector_count;
  char **command_arguments;
  int command_argument_count;
};

typedef void (*CommandInvocationHandler)(CommandInvocation *invocation);
typedef void (*CommandNoArgumentsHandler)(DbRef player, DbRef cause, int key);
typedef void (*CommandUnparsedHandler)(DbRef player, char *unparsed);
typedef void (*CommandOneArgumentHandler)(DbRef player, DbRef cause, int key,
                                          char *argument);
typedef void (*CommandVectorHandler)(DbRef player, DbRef cause, int key,
                                     char *argument, char **vector,
                                     int vector_count);
typedef void (*CommandTwoArgumentsHandler)(DbRef player, DbRef cause, int key,
                                           char *first, char *second);
typedef void (*CommandTwoArgumentsVectorHandler)(DbRef player, DbRef cause,
                                                 int key, char *first,
                                                 char *second, char **vector,
                                                 int vector_count);
typedef void (*CommandTwoVectorsHandler)(DbRef player, DbRef cause, int key,
                                         char *argument, char **vector,
                                         int vector_count,
                                         char **command_arguments,
                                         int command_argument_count);

void command_invocation_call_no_arguments(CommandNoArgumentsHandler handler,
                                          CommandInvocation *invocation);
void command_invocation_call_unparsed(CommandUnparsedHandler handler,
                                      CommandInvocation *invocation);
void command_invocation_call_one_argument(CommandOneArgumentHandler handler,
                                          CommandInvocation *invocation);
void command_invocation_call_vector(CommandVectorHandler handler,
                                    CommandInvocation *invocation);
void command_invocation_call_two_arguments(CommandTwoArgumentsHandler handler,
                                           CommandInvocation *invocation);
void command_invocation_call_two_arguments_vector(
    CommandTwoArgumentsVectorHandler handler, CommandInvocation *invocation);
void command_invocation_call_two_vectors(CommandTwoVectorsHandler handler,
                                         CommandInvocation *invocation);

#define DEFINE_COMMAND_ADAPTER(function)                                                \
  static void function##_command_adapter(CommandInvocation *invocation) {               \
    _Generic((function),                                                                \
        CommandNoArgumentsHandler: command_invocation_call_no_arguments,                \
        CommandUnparsedHandler: command_invocation_call_unparsed,                       \
        CommandOneArgumentHandler: command_invocation_call_one_argument,                \
        CommandVectorHandler: command_invocation_call_vector,                           \
        CommandTwoArgumentsHandler: command_invocation_call_two_arguments,              \
        CommandTwoArgumentsVectorHandler: command_invocation_call_two_arguments_vector, \
        CommandTwoVectorsHandler: command_invocation_call_two_vectors)(                 \
        function, invocation);                                                          \
  }
