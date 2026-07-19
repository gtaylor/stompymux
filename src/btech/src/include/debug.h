
/*
 * $Id: debug.h,v 1.1 2005/06/13 20:50:52 murrayma Exp $
 *
 * Last modified: Sun Nov  3 19:47:34 1996 fingon
 *
 */

#pragma once

#include "mux/server/platform.h"

void debug_allocfree(DbRef key, void **data, int selector);
void debug_list(DbRef player, void *data, char *buffer);
void debug_savedb(DbRef player, void *data, char *buffer);
void debug_shutdown(DbRef player, void *data, char *buffer);
