
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
int handleWeaponCrit(Mech *attacker, Mech *wounded, int hitloc, int critHit,
                     int critType, int LOS);
void HandleVTOLCrit(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                    int num);
void mech_main_weapon_destroy(Mech *mech);
void JamMainWeapon(Mech *mech);
void pickRandomWeapon(Mech *objMech, int wLoc, int *critNum, int wIgnoreJams);
void limitSpeedToCruise(Mech *objMech);
void DoVehicleStablizerCrit(Mech *objMech, int wLoc);
void DoTurretJamCrit(Mech *objMech);
void DoWeaponJamCrit(Mech *objMech, int wLoc);
void DoTurretLockCrit(Mech *objMech);
void DoWeaponDestroyedCrit(Mech *objAttacker, Mech *objMech, int wLoc, int LOS);
void DoTurretBlownOffCrit(Mech *objMech, Mech *objAttacker, int LOS);
void DoAmmunitionCrit(Mech *objMech, Mech *objAttacker, int wLoc, int LOS);
void DoCargoInfantryCrit(Mech *objMech, int wLoc);
void DoVehicleEngineHit(Mech *objMech, Mech *objAttacker);
void DoVehicleFuelTankCrit(Mech *objMech, Mech *objAttacker);
void DoVehicleCrewStunnedCrit(Mech *objMech);
void DoVehicleDriverCrit(Mech *objMech);
void DoVehicleSensorCrit(Mech *objMech);
void DoVehicleCommanderHit(Mech *objMech);
void DoVehicleCrewKilledCrit(Mech *objMech, Mech *objAttacker);
void DoVTOLCoPilotCrit(Mech *objMech);
void DoVTOLPilotHit(Mech *objMech);
void DoVTOLRotorDamagedCrit(Mech *objMech);
void DoVTOLTailRotorDamagedCrit(Mech *objMech);
void DoVTOLRotorDestroyedCrit(Mech *objMech, Mech *objAttacker, int LOS);
void StartVTOLCrash(Mech *objMech);
void HandleAdvFasaVehicleCrit(Mech *wounded, Mech *attacker, int LOS,
                              int hitloc, int num);
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
