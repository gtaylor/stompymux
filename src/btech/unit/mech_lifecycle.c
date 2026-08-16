/* State transitions for a BTech unit's lifecycle. */

#include "mech_lifecycle.h"

#include "checked_conversion.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"

#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "crit_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_stagger.h"
#include "mech_tag_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "special_object.h"

void mech_power_up(Mech *mech) {
  mech_status_set(&mech->rd.status, MECH_STATUS_STARTED);
  ((mech)->rd.turndamage) = 0;
  mech_update_recycling(mech);
  ((mech)->rd.num_seen) = 0;
  mech_start_seeing(mech);
}

void mech_power_down(Mech *mech) {
  if (!mech_is_destroyed(mech)) {
    mech_update_recycling(mech);
    ((mech)->rd.speed) = 0.0;
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_HEATCUTOFF);
    mech_status_clear(&mech->rd.status, (MechStatus)(MECH_STATUS_STARTED |
                                                     MECH_STATUS_MASC_ENABLED));
    mech_status2_clear(
        &mech->rd.status2,
        (MechStatus2)(MECH_STATUS2_ECM_ENABLED | MECH_STATUS2_ECCM_ENABLED |
                      MECH_STATUS2_PER_ECM_ENABLED |
                      MECH_STATUS2_PER_ECCM_ENABLED |
                      MECH_STATUS2_ANGEL_ECM_ENABLED |
                      MECH_STATUS2_ANGEL_ECCM_ENABLED |
                      MECH_STATUS2_NULLSIGSYS_ON | MECH_STATUS2_STH_ARMOR_ON));
    ((mech)->rd.desired_speed) = 0.0;
  }
  mech_pilot_dbref_set(mech, -1);
  mech_target_dbref_set(mech, -1);
  mech_event_cancel(mech, EVENT_STARTUP);
  mech_status2_clear(&mech->rd.status2, MECH_STATUS2_SLITE_ON);
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_LIT);
  mech_event_cancel(mech, EVENT_MOVEMODE);
  mech_status2_clear(&mech->rd.status2, MECH_STATUS2_MOVE_MODES);
  mech_event_cancel(mech, EVENT_JUMP);
  mech_event_cancel(mech, EVENT_MOVE);
  ((mech)->rd.masc_value) = 0;
  mech_event_cancel(mech, EVENT_STAND);
  mech_event_cancel(mech, EVENT_JUMPSTABIL);
  mech_event_cancel(mech, EVENT_TAKEOFF);
  mech_event_cancel(mech, EVENT_HIDE);
  mech_stop_digging(mech);
  mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
  mech_tag_stop(mech);
  mech_drop_club(mech);
  mech_event_cancel(mech, EVENT_MASC_FAIL);
  mech_charge_target_dbref_set(mech, -1);
  bsuit_swarm_stop(mech, 0);
  ((mech)->rd.scharge_value) = 0;
  if (((mech)->rd.carrying) > 0) {
    char empty_argument[] = "";
    mech_dropoff(GOD, mech, empty_argument);
  }
}

void mech_mark_destroyed(Mech *mech) {
  if (mech_pilot_is_unconscious(mech)) {
    mech_status_clear(&mech->rd.status, (MechStatus)(MECH_STATUS_BLINDED |
                                                     MECH_STATUS_UNCONSCIOUS));
    mech_notify(mech, MECHALL,
                "The mech was destroyed while pilot was unconscious!");
  }
  mech_status_clear(&mech->rd.status, MECH_STATUS_BLINDED);
  mech_power_down(mech);
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  mech_stop_stagger_check(mech);
  mech_stagger_tracking_reset(mech);
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_JELLIED);
  mech_status_set(&mech->rd.status, MECH_STATUS_DESTROYED);
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_MECH_STUNNED);
  bsuit_swarmers_stop(
      btech_context_get_map(mech->xcode.context, mech->mapindex), mech, 1);
  mech_events_cancel_all(mech);
  if ((((mech)->ud.type) == CLASS_MECH && mech_is_jumping(mech)) ||
      (((mech)->ud.type) != CLASS_MECH &&
       ((mech)->pd.z) > mech_upper_surface_elevation(mech))) {
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  }
}

void mech_destroy_and_place(Mech *mech) {
  mech_mark_destroyed(mech);
  ((mech)->rd.verticalspeed) = 0.0;
  if (mech_real_terrain_get(mech) == WATER ||
      mech_real_terrain_get(mech) == ICE) {
    ((mech)->pd.z) = clamp_int_to_short(mech_position_surface_elevation(mech));
  } else if (mech_real_terrain_get(mech) == BRIDGE) {
    if (((mech)->pd.z) >= mech_upper_surface_elevation(mech)) {
      ((mech)->pd.z) = clamp_int_to_short(mech_upper_surface_elevation(mech));
    } else {
      ((mech)->pd.z) = clamp_int_to_short(mech_lower_surface_elevation(mech));
    }
  } else {
    ((mech)->pd.z) = clamp_int_to_short(mech_position_surface_elevation(mech));
  }
  ((mech)->pd.fz) = (float)ZSCALE * ((mech)->pd.z);
}

bool mech_has_pilot(const Mech *mech) {
  return (mech_pilot_dbref(mech) > 0 &&
          game_object_location(mech->xcode.context->database,
                               mech_pilot_dbref(mech)) == mech->mynum) != 0;
}

bool mech_has_active_pilot(const Mech *mech) {
  return (mech_has_pilot(mech) && (is_connected(mech->xcode.context->database,
                                                mech_pilot_dbref(mech)) ||
                                   !is_player(mech->xcode.context->database,
                                              mech_pilot_dbref(mech)))) != 0;
}

bool mech_has_gunner(const Mech *mech) {
  return ((mech->xcode.context->combat_overrides.pilot &&
           mech_gunner_dbref(mech) > 0) ||
          (!mech->xcode.context->combat_overrides.pilot &&
           mech_has_pilot(mech))) != 0;
}

bool mech_has_active_gunner(const Mech *mech) {
  if (!mech->xcode.context->combat_overrides.pilot) {
    return mech_has_active_pilot(mech);
  }
  return (mech_gunner_dbref(mech) > 0 &&
          (is_connected(mech->xcode.context->database,
                        mech_gunner_dbref(mech)) ||
           !is_player(mech->xcode.context->database,
                      mech_gunner_dbref(mech)))) != 0;
}

void mech_max_speed_set(Mech *mech, float speed) {
  ((mech)->ud.maxspeed) = speed;
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_SPEED_OK);
  mech_speed_correct(mech);
}

void mech_max_speed_lower(Mech *mech, float amount) {
  mech_max_speed_set(mech, ((mech)->ud.maxspeed) - amount);
}

void mech_max_speed_divide(Mech *mech, float divisor) {
  mech_max_speed_set(mech, ((mech)->ud.maxspeed) / divisor);
}

bool mech_can_jump(const Mech *mech) {
  return (!mech_event_count(mech, EVENT_JUMPSTABIL) &&
          !mech_is_jumping(mech)) != 0;
}

void mech_maybe_move(Mech *mech) {
  if (!mech_event_count(mech, EVENT_MOVE) && mech_is_started(mech) &&
      (!mech_is_fallen(mech) || ((mech)->ud.type) == CLASS_MECH)) {
    mech_event_schedule(mech, EVENT_MOVE,
                        mech_is_aerospace_unit(mech) ? aero_move_event
                                                     : mech_move_event,
                        MOVE_TICK, 0);
  }
}

void mech_update_recycling(Mech *mech) {
  if (mech_is_started(mech) && !mech_is_destroyed(mech) &&
      mech->rd.last_weapon_recycle != mech->xcode.context->events->tick) {
    mech_weapon_recycle_update(mech);
  }
}

void mech_set_recycle_part(Mech *mech, int section, int critical, int value) {
  mech_update_recycling(mech);
  mech_critical_data_set(mech, section, critical, value);
}

void mech_set_recycle_limb(Mech *mech, int section, int value) {
  mech_update_recycling(mech);
  mech_section_recycle_ticks_set(mech, section, value);
}

void mech_make_fall(Mech *mech) {
  mech_status_set(&mech->rd.status, MECH_STATUS_FALLEN);
  mech_status_clear(&mech->rd.status, (MechStatus)(MECH_STATUS_TORSO_RIGHT |
                                                   MECH_STATUS_TORSO_LEFT |
                                                   MECH_STATUS_FLIPPED_ARMS));
  mark_for_los_update(mech);
  mech_flood(mech);
  mech_event_cancel(mech, EVENT_STAND);
  mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
  mech_status_clear(&mech->rd.status, MECH_STATUS_HULLDOWN);
  if (btech_context_stagger_mode(mech_context(mech))) {
    mech_stagger_damage_clear(mech);
  }
}

void mech_make_stand(Mech *mech) {
  mech_status_clear(&mech->rd.status, MECH_STATUS_FALLEN);
  mark_for_los_update(mech);
}

void mech_start_seeing(Mech *mech) {
  mech_event_schedule(mech, EVENT_PLOS, mech_plos_event, INITIAL_PLOS_TICK, 0);
}

void mech_continue_flying(Mech *mech) {
  if (mech_is_aerospace_unit(mech) || ((mech)->ud.move) == MOVE_VTOL) {
    mech_status_clear(&mech->rd.status, MECH_STATUS_LANDED);
    ((mech)->pd.z) += 1;
    ((mech)->pd.fz) = (float)ZSCALE * ((mech)->pd.z);
    mech_event_cancel(mech, EVENT_MOVE);
  }
}

void mech_drop_club(Mech *mech) {
  if ((((mech)->ud.sections)[RARM].specials & CARRYING_CLUB) ||
      (((mech)->ud.sections)[LARM].specials & CARRYING_CLUB)) {
    ((mech)->ud.sections)[RARM].specials &= ~CARRYING_CLUB;
    ((mech)->ud.sections)[LARM].specials &= ~CARRYING_CLUB;
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
  return (((mech)->ud.type) == CLASS_VTOL &&
          mech->xcode.context->configuration->btech_nofusionvtolfuel &&
          !(((mech)->rd.specials) & ICE_TECH)) != 0;
}

size_t mech_storage_size(void) { return sizeof(Mech); }

Mech *mech_temporary_create(BtechContext *context) {
  Mech *mech = checked_storage_try_allocate_array(1, sizeof(*mech));
  if (mech == nullptr)
    return nullptr;
  mech->xcode = (BtechSpecialObject){
      .type = GTYPE_MECH,
      .size = sizeof(*mech),
      .context = context,
  };
  return mech;
}

Mech *mech_temporary_clone(const Mech *source) {
  if (source == nullptr)
    return nullptr;
  Mech *mech = checked_storage_try_allocate(sizeof(*mech));
  if (mech != nullptr)
    memcpy(mech, source, sizeof(*mech));
  return mech;
}

void mech_temporary_destroy(Mech *mech) { free(mech); }
