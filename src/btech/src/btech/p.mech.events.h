
/*
   p.mech.events.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:48 CET 1999 from mech.events.c */

#pragma once

/* mech.events.c */
void mech_standfail_event(MuxEvent *e);
void mech_fall_event(MuxEvent *e);
void mech_lock_event(MuxEvent *e);
void mech_stabilizing_event(MuxEvent *e);
void mech_jump_event(MuxEvent *e);
void mech_recovery_event(MuxEvent *e);
void mech_recycle_event(MuxEvent *e);
void ProlongUncon(MECH *mech, int len);
void MaybeRecycle(MECH *mech, int wticks);
void mech_lateral_event(MuxEvent *e);
void mech_move_event(MuxEvent *e);
void mech_stand_event(MuxEvent *e);
void mech_plos_event(MuxEvent *e);
void aero_move_event(MuxEvent *e);
void very_fake_func(MuxEvent *e);
void mech_crewstun_event(MuxEvent *e);
void unstun_crew_event(MuxEvent *e);
void mech_unjam_ammo_event(MuxEvent *objEvent);
void check_stagger_event(MuxEvent *event);
#ifdef BT_MOVEMENT_MODES
void mech_movemode_event(MuxEvent *e);
#endif
int calcStaggerBTHMod(MECH *mech);
int calcNewStaggerBTHMod(MECH *mech, int staggerLevel);
void mech_staggercheck_heartbeat(MECH *mech);
