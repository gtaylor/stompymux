
/*
         p.crit.h

         Automatically created by protomaker (C) 1998 Markus Stenberg
   (fingon@iki.fi) Protomaker is actually only a wrapper script for cproto, but
   well.. I like fancy headers and stuff :)
         */

/* Generated at Wed Feb 17 23:36:30 CET 1999 from crit.c */

#pragma once

#include "mux/server/platform.h"

/* crit.c */
void mech_speed_correct(Mech *mech);
void mech_explosion_apply(Mech *wounded, Mech *attacker);
int mech_weapon_critical_handle(Mech *attacker, Mech *wounded, int hitloc,
                                int critHit, int critType, int LOS);
void mech_vtol_critical_handle(Mech *wounded, Mech *attacker, int LOS,
                               int hitloc, int num);
void mech_main_weapon_destroy(Mech *mech);
void mech_main_weapon_jam(Mech *mech);
void mech_random_weapon_select(Mech *objMech, int wLoc, int *critNum,
                               int wIgnoreJams);
void mech_speed_limit_to_cruise(Mech *objMech);
void mech_vehicle_stabilizer_critical_apply(Mech *objMech, int wLoc);
void mech_turret_jam_critical_apply(Mech *objMech);
void mech_weapon_jam_critical_apply(Mech *objMech, int wLoc);
void mech_turret_lock_critical_apply(Mech *objMech);
void mech_weapon_destroyed_critical_apply(Mech *objAttacker, Mech *objMech,
                                          int wLoc, int LOS);
void mech_turret_blown_off_critical_apply(Mech *objMech, Mech *objAttacker,
                                          int LOS);
void mech_ammunition_critical_apply(Mech *objMech, Mech *objAttacker, int wLoc,
                                    int LOS);
void mech_cargo_infantry_critical_apply(Mech *objMech, int wLoc);
void mech_vehicle_engine_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vehicle_fuel_tank_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vehicle_crew_stun_critical_apply(Mech *objMech);
void mech_vehicle_driver_critical_apply(Mech *objMech);
void mech_vehicle_sensor_critical_apply(Mech *objMech);
void mech_vehicle_commander_critical_apply(Mech *objMech);
void mech_vehicle_crew_killed_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vtol_copilot_critical_apply(Mech *objMech);
void mech_vtol_pilot_critical_apply(Mech *objMech);
void mech_vtol_rotor_damaged_critical_apply(Mech *objMech);
void mech_vtol_tail_rotor_critical_apply(Mech *objMech);
void mech_vtol_rotor_destroyed_critical_apply(Mech *objMech, Mech *objAttacker,
                                              int LOS);
void mech_vtol_crash_start(Mech *objMech);
void mech_advanced_vehicle_critical_handle(Mech *wounded, Mech *attacker,
                                           int LOS, int hitloc, int num);
void mech_fasa_vehicle_critical_handle(Mech *wounded, Mech *attacker, int LOS,
                                       int hitloc, int num);
void mech_vehicle_critical_handle(Mech *wounded, Mech *attacker, int LOS,
                                  int hitloc, int num);
int HandleMechCrit(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                   int critHit, int critType, int critData);
void mech_critical_handle(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                          int num);
void mech_arm_actuator_criticals_normalize(Mech *objMech, int wLoc,
                                           int wCritType);
void mech_leg_actuator_criticals_normalize(Mech *objMech, int wLoc,
                                           int wCritType);
void mech_section_actuator_criticals_normalize(Mech *objMech, int wLoc);
void mech_actuator_criticals_normalize(Mech *objMech);
