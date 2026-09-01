/* Private helpers shared by channel command implementations. */

#pragma once

#include "mux/communication/comsys.h"

void comsys_process_alias_command(EvaluationContext *evaluation, DbRef player,
                                  const char *arg1, char *arg2);
void comsys_channel_printf(EvaluationContext *evaluation,
                           struct Channel *channel, const char *messfmt, ...)
    __attribute__((format(printf, 3, 4)));
void comsys_leave_channel(EvaluationContext *evaluation, DbRef player,
                          struct Channel *channel);
void comsys_show_channel_who(EvaluationContext *evaluation, DbRef player,
                             struct Channel *channel);
void comsys_delete_channel_alias(EvaluationContext *evaluation, DbRef player,
                                 char *channel);
typedef struct ChannelAccessRequest {
  EvaluationContext *evaluation;
  DbRef player;
  long access;
  struct Channel *channel;
} ChannelAccessRequest;

int comsys_test_access(const ChannelAccessRequest *request);
void comsys_disconnect_channel(EvaluationContext *evaluation, DbRef player,
                               char *channel);
void comsys_send_channel_message(EvaluationContext *evaluation,
                                 struct Channel *channel, const char *message);
void comsys_list_channels(EvaluationContext *evaluation, DbRef player);
