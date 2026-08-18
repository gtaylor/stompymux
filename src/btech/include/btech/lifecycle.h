/** @file
 * BTech runtime producer lifecycle.
 */

#pragma once

typedef struct BtechContext BtechContext;

/**
 * Starts periodic BTech runtime events.
 *
 * @param[in,out] context BTech runtime context.
 */
void btech_heartbeat_start(BtechContext *context);

/**
 * Stops periodic BTech runtime events.
 *
 * @param[in,out] context BTech runtime context.
 */
void btech_heartbeat_stop(BtechContext *context);
