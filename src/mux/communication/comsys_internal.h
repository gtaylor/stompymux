/* Private helpers shared by channel command implementations. */

#pragma once

#include "mux/communication/comsys.h"

void comsys_process_alias_command(EvaluationContext *evaluation, DbRef player,
                                  char *alias, char *argument);
void comsys_channel_printf(EvaluationContext *evaluation,
                           struct channel *channel, const char *format, ...)
    __attribute__((format(printf, 3, 4)));
void comsys_leave_channel(EvaluationContext *evaluation, DbRef player,
                          struct channel *channel);
void comsys_show_channel_who(EvaluationContext *evaluation, DbRef player,
                             struct channel *channel);
void comsys_delete_channel_alias(EvaluationContext *evaluation, DbRef player,
                                 char *channel);
int comsys_test_access(EvaluationContext *evaluation, DbRef player, long access,
                       struct channel *channel);
void comsys_disconnect_channel(EvaluationContext *evaluation, DbRef player,
                               char *channel);
char *comsys_channel_from_alias(EvaluationContext *evaluation, DbRef player,
                                char *alias);
void comsys_send_channel_message(EvaluationContext *evaluation,
                                 struct channel *channel, char *message);
void comsys_list_channels(EvaluationContext *evaluation, DbRef player);
