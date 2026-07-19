
/*
   p.debug.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:36 CET 1999 from debug.c */

#pragma once

typedef struct BtechContext BtechContext;

/* debug.c */
void debug_list(DbRef player, void *data, char *buffer);
void debug_savedb(DbRef player, void *data, char *buffer);
void debug_memory(DbRef player, void *data, char *buffer);
void ShutDownMap(BtechContext *context, DbRef player, DbRef mapnumber);
void debug_shutdown(DbRef player, void *data, char *buffer);
void debug_setvrt(DbRef player, void *data, char *buffer);
