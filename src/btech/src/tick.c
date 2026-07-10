/*
 *
 * Copyright (c) 2005 Martin Murray
 *
 * This is much better than what we had.
 *
 */

#include "config.h"

#include "autopilot.h"
#include "debug.h"
#include "glue_types.h"
#include "mech.h"
#include "rbtree.h"
#include <event.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

static struct event heartbeat_ev;
static struct timeval heartbeat_tv = {1, 0};
static int heartbeat_running = 0;
unsigned int global_tick = 0;
extern rbtree xcode_tree;

void heartbeat_run(int fd, short event, void *arg);

void heartbeat_init() {
  if (heartbeat_running)
    return;
  dprintk("hearbeat initialized, %ds timeout.", (int)heartbeat_tv.tv_sec);
  evtimer_set(&heartbeat_ev, heartbeat_run, NULL);
  evtimer_add(&heartbeat_ev, &heartbeat_tv);
  heartbeat_running = 1;
}

void heartbeat_stop() {
  if (!heartbeat_running)
    return;
  evtimer_del(&heartbeat_ev);
  dprintk("heartbeat stopped.\n");
  heartbeat_running = 0;
}

void mech_heartbeat(MECH *);
void auto_heartbeat(AUTO *);

static int heartbeat_dispatch(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  switch (xcode_obj->type) {
  case GTYPE_MECH:
    mech_heartbeat((MECH *)xcode_obj);
    break;

  case GTYPE_AUTO:
    auto_heartbeat((AUTO *)xcode_obj);
    break;

  default:
    break;
  }

  return 1;
}

void heartbeat_run(int fd, short event, void *arg) {
  evtimer_add(&heartbeat_ev, &heartbeat_tv);
  rb_walk(xcode_tree, WALK_INORDER, heartbeat_dispatch, NULL);
  global_tick++;
}
