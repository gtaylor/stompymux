#ifndef BTMUX_DNSCHILD_H
#define BTMUX_DNSCHILD_H

#include "interface.h"

int dnschild_init(void);
void *dnschild_request(DESC *d);
void dnschild_destruct(void);
void dnschild_kill(void *arg);

#endif
