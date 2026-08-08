#pragma once

typedef struct Mech Mech;
typedef struct MuxEvent MuxEvent;

void mech_standfail_event(MuxEvent *event);
void mech_fall_event(MuxEvent *event);
void mech_lock_event(MuxEvent *event);
void mech_stabilizing_event(MuxEvent *event);
void mech_jump_event(MuxEvent *event);
void mech_recovery_event(MuxEvent *event);
void mech_recycle_event(MuxEvent *event);
void mech_unconsciousness_extend(Mech *mech, int ticks);
void mech_lateral_event(MuxEvent *event);
void mech_move_event(MuxEvent *event);
void mech_stand_event(MuxEvent *event);
void mech_plos_event(MuxEvent *event);
void aero_move_event(MuxEvent *event);

void mech_crewstun_event(MuxEvent *event);
void unstun_crew_event(MuxEvent *event);
void mech_unjam_ammo_event(MuxEvent *event);
void check_stagger_event(MuxEvent *event);
#ifdef BT_MOVEMENT_MODES
void mech_movemode_event(MuxEvent *event);
#endif
int mech_stagger_modifier(Mech *mech);
int mech_stagger_modifier_at_level(Mech *mech, int stagger_level);
void mech_staggercheck_heartbeat(Mech *mech);
