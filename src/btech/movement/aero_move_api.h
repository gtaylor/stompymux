/* Declares the BattleTech aerospace move API. */

#pragma once

#include "mux/server/platform.h"

/* aero.move.c */
void aero_takeoff(DbRef player, void *data, const char *buffer);
typedef struct DropshipExhaustBlastRequest {
  Mech *dropship;
  const char *direct_message;
  const char *direct_observer_message;
  const char *nearby_message;
  const char *nearby_observer_message;
  const char *tree_message;
  int damage;
} DropshipExhaustBlastRequest;
void dropship_exhaust_blast(const DropshipExhaustBlastRequest *request);
void aero_land(DbRef player, void *data, const char *buffer);
void aero_control_effect(Mech *mech);
void dropship_bridge_hit(Mech *mech);
void aero_heading_update(Mech *mech);
double length_hypotenuse(double x, double y);
double my_sqrtm(double x, double y);
void aero_speed_update(Mech *mech);
bool aero_fuel_check(Mech *mech);
void aero_update(Mech *mech);
void aero_thrust(DbRef player, void *data, char *arg);
void aero_vheading(DbRef player, void *data, char *arg, int flag);
void aero_climb(DbRef player, Mech *mech, char *arg);
void aero_dive(DbRef player, Mech *mech, char *arg);
int aero_landing_zone_check(Mech *mech, int x, int y);
const char *aero_landing_reason(int index);
void dropship_land_warning(Mech *mech, int serious);
void aero_checklz(DbRef player, Mech *mech, char *buffer);
