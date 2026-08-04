/* lifecycle.h - BTech runtime producer lifecycle. */

#pragma once

typedef struct BtechContext BtechContext;

void btech_heartbeat_start(BtechContext *context);
void btech_heartbeat_stop(BtechContext *context);
