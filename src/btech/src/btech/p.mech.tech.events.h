
/*
   p.mech.tech.events.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:00 CET 1999 from mech.tech.events.c */

#pragma once

/* mech.tech.events.c */
void mux_event_tickmech_removesection(MuxEvent *e);
void mux_event_tickmech_removegun(MuxEvent *e);
void mux_event_tickmech_removepart(MuxEvent *e);
void mux_event_tickmech_repairarmor(MuxEvent *e);
void mux_event_tickmech_repairinternal(MuxEvent *e);
void mux_event_tickmech_reattach(MuxEvent *e);
void mux_event_tickmech_replacesuit(MuxEvent *e);
void mux_event_tickmech_replacegun(MuxEvent *e);
void mux_event_tickmech_repairgun(MuxEvent *e);
void event_mech_repairenhcrit(MuxEvent *e);
void mux_event_tickmech_repairpart(MuxEvent *e);
void mux_event_tickmech_reload(MuxEvent *e);
void mux_event_tickmech_mountbomb(MuxEvent *e);
void mux_event_tickmech_umountbomb(MuxEvent *e);
void mux_event_tickmech_replacesuit(MuxEvent *e);
