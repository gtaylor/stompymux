/* State transitions for a BTech unit's lifecycle. */

#include "mech_lifecycle.h"

#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "crit_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_stagger.h"
#include "mech_tag_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "registry_api.h"

void mech_power_up(Mech *mech) {
  MechStatus(mech) |= STARTED;
  MechTurnDamage(mech) = 0;
  mech_update_recycling(mech);
  MechNumSeen(mech) = 0;
  mech_start_seeing(mech);
}

void mech_power_down(Mech *mech) {
  if (!Destroyed(mech)) {
    mech_update_recycling(mech);
    MechSpeed(mech) = 0.0;
    MechCritStatus(mech) &= ~HEATCUTOFF;
    MechStatus(mech) &= ~(STARTED | MASC_ENABLED);
    MechStatus2(mech) &= ~(ECM_ENABLED | ECCM_ENABLED | PER_ECM_ENABLED |
                           PER_ECCM_ENABLED | ANGEL_ECM_ENABLED |
                           ANGEL_ECCM_ENABLED | NULLSIGSYS_ON | STH_ARMOR_ON);
    MechDesiredSpeed(mech) = 0.0;
  }
  MechPilot(mech) = -1;
  MechTarget(mech) = -1;
  mech_event_cancel(mech, EVENT_STARTUP);
  MechStatus2(mech) &= ~SLITE_ON;
  MechCritStatus(mech) &= ~SLITE_LIT;
  mech_event_cancel(mech, EVENT_MOVEMODE);
  MechStatus2(mech) &= ~MOVE_MODES;
  mech_event_cancel(mech, EVENT_JUMP);
  mech_event_cancel(mech, EVENT_MOVE);
  MechMASCCounter(mech) = 0;
  mech_event_cancel(mech, EVENT_STAND);
  mech_event_cancel(mech, EVENT_JUMPSTABIL);
  mech_event_cancel(mech, EVENT_TAKEOFF);
  mech_event_cancel(mech, EVENT_HIDE);
  mech_stop_digging(mech);
  mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
  mech_tag_stop(mech);
  mech_drop_club(mech);
  mech_event_cancel(mech, EVENT_MASC_FAIL);
  MechChargeTarget(mech) = -1;
  StopSwarming(mech, 0);
  MechSChargeCounter(mech) = 0;
  if (MechCarrying(mech) > 0) {
    mech_dropoff(GOD, mech, "");
  }
}

void mech_mark_destroyed(Mech *mech) {
  if (Uncon(mech)) {
    MechStatus(mech) &= ~(BLINDED | UNCONSCIOUS);
    mech_notify(mech, MECHALL,
                "The mech was destroyed while pilot was unconscious!");
  }
  MechStatus(mech) &= ~BLINDED;
  mech_power_down(mech);
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  mech_stop_stagger_check(mech);
  StaggerDamage(mech) = 0;
  MechCritStatus(mech) &= ~JELLIED;
  MechStatus(mech) |= DESTROYED;
  MechCritStatus(mech) &= ~MECH_STUNNED;
  StopBSuitSwarmers(btech_context_get_map(mech->xcode.context, mech->mapindex),
                    mech, 1);
  mech_events_cancel_all(mech);
  if ((MechType(mech) == CLASS_MECH && Jumping(mech)) ||
      (MechType(mech) != CLASS_MECH &&
       MechZ(mech) > MechUpperElevation(mech))) {
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  }
}

void mech_destroy_and_place(Mech *mech) {
  mech_mark_destroyed(mech);
  MechVerticalSpeed(mech) = 0.0;
  if (mech_real_terrain_get(mech) == WATER ||
      mech_real_terrain_get(mech) == ICE) {
    MechZ(mech) = -MechElev(mech);
  } else if (mech_real_terrain_get(mech) == BRIDGE) {
    if (MechZ(mech) >= MechUpperElevation(mech)) {
      MechZ(mech) = MechUpperElevation(mech);
    } else {
      MechZ(mech) = MechLowerElevation(mech);
    }
  } else {
    MechZ(mech) = MechElev(mech);
  }
  MechFZ(mech) = ZSCALE * MechZ(mech);
}

bool mech_has_pilot(const Mech *mech) {
  return MechPilot(mech) > 0 &&
         game_object_location(mech->xcode.context->database, MechPilot(mech)) ==
             mech->mynum;
}

bool mech_has_active_pilot(const Mech *mech) {
  return mech_has_pilot(mech) &&
         (is_connected(mech->xcode.context->database, MechPilot(mech)) ||
          !is_player(mech->xcode.context->database, MechPilot(mech)));
}

bool mech_has_gunner(const Mech *mech) {
  return (mech->xcode.context->combat_overrides.pilot && GunPilot(mech) > 0) ||
         (!mech->xcode.context->combat_overrides.pilot && mech_has_pilot(mech));
}

bool mech_has_active_gunner(const Mech *mech) {
  if (!mech->xcode.context->combat_overrides.pilot) {
    return mech_has_active_pilot(mech);
  }
  return GunPilot(mech) > 0 &&
         (is_connected(mech->xcode.context->database, GunPilot(mech)) ||
          !is_player(mech->xcode.context->database, GunPilot(mech)));
}

void mech_max_speed_set(Mech *mech, float speed) {
  MechMaxSpeed(mech) = speed;
  MechCritStatus(mech) &= ~SPEED_OK;
  correct_speed(mech);
}

void mech_max_speed_lower(Mech *mech, float amount) {
  mech_max_speed_set(mech, MechMaxSpeed(mech) - amount);
}

void mech_max_speed_divide(Mech *mech, float divisor) {
  mech_max_speed_set(mech, MechMaxSpeed(mech) / divisor);
}

bool mech_can_jump(const Mech *mech) {
  return !mech_event_count(mech, EVENT_JUMPSTABIL) && !Jumping(mech);
}

void mech_maybe_move(Mech *mech) {
  if (!mech_event_count(mech, EVENT_MOVE) && Started(mech) &&
      (!Fallen(mech) || MechType(mech) == CLASS_MECH)) {
    mech_event_schedule(mech, EVENT_MOVE,
                        is_aero(mech) ? aero_move_event : mech_move_event,
                        MOVE_TICK, 0);
  }
}

void mech_update_recycling(Mech *mech) {
  if (Started(mech) && !Destroyed(mech) &&
      mech->rd.last_weapon_recycle != mech->xcode.context->events->tick) {
    recycle_weaponry(mech);
  }
}

void mech_set_recycle_part(Mech *mech, int section, int critical, int value) {
  mech_update_recycling(mech);
  SetPartData(mech, section, critical, value);
}

void mech_set_recycle_limb(Mech *mech, int section, int value) {
  mech_update_recycling(mech);
  mech->ud.sections[section].recycle = value;
}

void mech_make_fall(Mech *mech) {
  MechStatus(mech) |= FALLEN;
  MechStatus(mech) &= ~(TORSO_RIGHT | TORSO_LEFT | FLIPPED_ARMS);
  MarkForLOSUpdate(mech);
  mech_flood(mech);
  mech_event_cancel(mech, EVENT_STAND);
  mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
  MechStatus(mech) &= ~HULLDOWN;
  if (btech_context_stagger_mode(mech_context(mech))) {
    mech_stagger_damage_clear(mech);
  }
}

void mech_make_stand(Mech *mech) {
  MechStatus(mech) &= ~FALLEN;
  MarkForLOSUpdate(mech);
}

void mech_start_seeing(Mech *mech) {
#ifdef ADVANCED_LOS
  mech_event_schedule(mech, EVENT_PLOS, mech_plos_event, INITIAL_PLOS_TICK, 0);
#else
  (void)mech;
#endif
}

void mech_continue_flying(Mech *mech) {
  if (is_aero(mech) || MechMove(mech) == MOVE_VTOL) {
    MechStatus(mech) &= ~LANDED;
    MechZ(mech) += 1;
    MechFZ(mech) = ZSCALE * MechZ(mech);
    mech_event_cancel(mech, EVENT_MOVE);
  }
}

void mech_drop_club(Mech *mech) {
  if ((MechSections(mech)[RARM].specials & CARRYING_CLUB) ||
      (MechSections(mech)[LARM].specials & CARRYING_CLUB)) {
    MechSections(mech)[RARM].specials &= ~CARRYING_CLUB;
    MechSections(mech)[LARM].specials &= ~CARRYING_CLUB;
    mech_notify(mech, MECHALL, "Your club falls to the ground and shatters.");
    mech_los_broadcast(mech, "'s club falls to the ground and shatters.");
  }
}

void mech_template_state_reset(Mech *mech) {
  mech->brief = 1;
  memset(&mech->rd, 0, sizeof(mech->rd));
  memset(&mech->ud, 0, sizeof(mech->ud));
}

void mech_communications_clear(Mech *mech) {
  memset(mech->tic, 0, sizeof(mech->tic));
  memset(mech->freq, 0, sizeof(mech->freq));
  memset(mech->freqmodes, 0, sizeof(mech->freqmodes));
  memset(mech->chantitle, 0, sizeof(mech->chantitle));
}

bool mech_aero_has_free_fuel(const Mech *mech) {
  return MechType(mech) == CLASS_VTOL &&
         mech->xcode.context->configuration->btech_nofusionvtolfuel &&
         !(MechSpecials(mech) & ICE_TECH);
}
