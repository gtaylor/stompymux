/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include "btech/context.h"
#include "legacy_macros.h"
#include "mech_api_types.h"
#include "mech_combat_api.h"
#include "mech_contacts_api.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_maps_api.h"
#include "mech_move_api.h"
#include "mech_scan_api.h"
#include "mech_status_api.h"
#include "mech_targeting_api.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "turret.h"

typedef struct TurretTargetingScope {
  MechTargetingOverride targeting;
} TurretTargetingScope;

static Mech *turret_parent_mech(DbRef player, Turret *tur,
                                bool require_gunner) {
  Mech *mech = btech_context_find_object(tur->xcode.context, tur->parent);

  if (!btech_context_is_mech(tur->xcode.context, tur->parent)) {
    notify(btech_context_evaluation(tur->xcode.context), player,
           "Error: Turret's parentage is unknown.");
    return nullptr;
  }
  if (!require_gunner)
    return mech;
  if (tur->gunner < 0) {
    notify(btech_context_evaluation(tur->xcode.context), player,
           "The turret hasn't been initialized yet!");
    return nullptr;
  }
  if (player != tur->gunner) {
    notify(btech_context_evaluation(tur->xcode.context), player,
           "You aren't the registered gunner! Go 'way!");
    return nullptr;
  }
  if (player == mech_pilot_dbref(mech)) {
    notify(btech_context_evaluation(tur->xcode.context), player,
           "You'll pilot and gun at once? Yah right :P");
    return nullptr;
  }
  return mech;
}

static void turret_targeting_scope_enter(TurretTargetingScope *scope,
                                         Turret *tur, Mech *mech) {
  if (tur->gunner > 0)
    btech_context_combat_pilot_override_set(tur->xcode.context, tur->gunner);
  mech_targeting_override_begin(mech, &scope->targeting, tur->target,
                                tur->targx, tur->targy, tur->targz,
                                tur->lockmode);
  btech_context_combat_arcs_override_set(tur->xcode.context, tur->arcs);
}

static void turret_targeting_scope_leave(TurretTargetingScope *scope,
                                         Turret *tur, Mech *mech) {
  int target_x;
  int target_y;
  int target_z;

  btech_context_combat_pilot_override_set(tur->xcode.context, 0);
  mech_targeting_override_end(mech, &scope->targeting, &tur->target, &target_x,
                              &target_y, &target_z, &tur->lockmode);
  tur->targx = target_x;
  tur->targy = target_y;
  tur->targz = target_z;
  btech_context_combat_arcs_override_set(tur->xcode.context, 0);
}

void turret_addtic(DbRef player, void *data, char *buffer) {
  (void)player;
  (void)data;
  (void)buffer;
}

void turret_deltic(DbRef player, void *data, char *buffer) {
  (void)player;
  (void)data;
  (void)buffer;
}

void turret_listtic(DbRef player, void *data, char *buffer) {
  (void)player;
  (void)data;
  (void)buffer;
}

void turret_cleartic(DbRef player, void *data, char *buffer) {
  (void)player;
  (void)data;
  (void)buffer;
}

void turret_firetic(DbRef player, void *data, char *buffer) {
  (void)player;
  (void)data;
  (void)buffer;
}

void turret_bearing(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_bearing(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_eta(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_eta(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_findcenter(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  mech_findcenter(player, mech, buffer);
}

void turret_fireweapon(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_fireweapon(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_settarget(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_set_target(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_lrsmap(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_lrsmap(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_navigate(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_navigate(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_range(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_range(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_sight(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_sight(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_tacmap(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_tacmap(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_contacts(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_contacts(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_critstatus(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_critstatus(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_report(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_report(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_scan(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_scan(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_status(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_status(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

void turret_weaponspecs(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  Mech *mech = turret_parent_mech(player, tur, true);
  if (mech == nullptr)
    return;
  TurretTargetingScope scope;
  turret_targeting_scope_enter(&scope, tur, mech);
  mech_weaponspecs(player, mech, buffer);
  turret_targeting_scope_leave(&scope, tur, mech);
}

enum { TURRET_LIFECYCLE_ALLOC = 1 };

/* Alloc/free routine */
void turret_lifecycle_update(DbRef key, void **data, int selector) {
  Turret *new = *data;

  switch (selector) {
  case TURRET_LIFECYCLE_ALLOC:
    new->mynum = key;
    new->target = -1;
    new->targx = -1;
    new->targy = -1;
    break;
  }
}

void turret_initialize(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  if (turret_parent_mech(player, tur, false) == nullptr)
    return;
  DOCHECK_CONTEXT(
      tur->xcode.context,
      player != tur->gunner &&
          is_connected(tur->xcode.context->database, tur->gunner) &&
          game_object_location(tur->xcode.context->database, tur->gunner) ==
              game_object_location(tur->xcode.context->database, player),
      tprintf("You need %s to leave or disconnect first.",
              game_object_name(tur->xcode.context->database, tur->gunner)));
  DOCHECK_CONTEXT(tur->xcode.context, player == tur->gunner,
                  "You grap firmer hold on the joystick..");
  notify_except(
      btech_context_evaluation(tur->xcode.context), tur->mynum, NOTHING,
      tur->mynum,
      tprintf("%s initialized as gunner.",
              game_object_name(tur->xcode.context->database, player)));
  tur->gunner = player;
}

void turret_deinitialize(DbRef player, void *data, char *buffer) {
  Turret *tur = data;
  if (turret_parent_mech(player, tur, false) == nullptr)
    return;
  DOCHECK_CONTEXT(tur->xcode.context, player != tur->gunner,
                  "You aren't gunner!");
  notify_except(
      btech_context_evaluation(tur->xcode.context), tur->mynum, NOTHING,
      tur->mynum,
      tprintf("%s deinitialized as gunner.",
              game_object_name(tur->xcode.context->database, player)));
  tur->gunner = -1;
}
