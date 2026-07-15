
/*
   p.mech.pickup.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:54 CET 1999 from mech.pickup.c */

#pragma once

/* mech.pickup.c */
void mech_pickup(DbRef player, void *data, char *buffer);
void mech_attachcables(DbRef player, void *data, char *buffer);
void mech_detachcables(DbRef player, void *data, char *buffer);
void mech_dropoff(DbRef player, void *data, char *buffer);
