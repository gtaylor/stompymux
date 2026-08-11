/* Implements BattleTech movement mechanics for unit update. */

#include "btconfig.h"
#include "mech_update_api.h"

#include <stdio.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_charge_tracking_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_fire_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_motion_integration_api.h"
#include "mech_move_api.h"
#include "mech_movement_validation_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_towing_sync_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"

void mech_movement_update(Mech *mech) {
  BattleMap *mech_map = mech_movement_map_validate(mech);
  if (!mech_map)
    return;

  mech_charge_timeout_update(mech);

  MechMotionStep step;
  if (!mech_motion_integrate(mech, mech_map, &step))
    return;

  int last_z = mech_position_z(mech);
  mech_position_previous_capture(mech);
  mech_position_hex_sync_from_real(mech);

#ifdef ODDJUMP
  if (mech_jump_destination_was_overshot(mech)) {
    mech_jump_land(mech);
    mech_jump_overshoot_restore(mech, step.delta_x, step.delta_y);
  }
#endif

  DbRef previous_map = mech_map_dbref(mech);
  check_edge_of_map(mech);
  if (mech_map_dbref(mech) != previous_map)
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (previous_map != mech_map_dbref(mech) ||
      mech_position_previous_x(mech) != mech_position_x(mech) ||
      mech_position_previous_y(mech) != mech_position_y(mech)) {
    if (!mech || !mech_map) {
      char message_buffer[MBUF_SIZE];
      (void)snprintf(message_buffer, MBUF_SIZE,
                     "Invalide pointer (%s) in move_mech()",
                     (!mech       ? "mech"
                      : !mech_map ? "mech_map"
                                  : "weird...."));
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                         message_buffer);

      if (mech) {
        mech_notify(mech, MECHALL,
                    "You are on an invalid map! Map index reset!");
        mech_cocoon_integrity_set(mech, 0);
        if (mech_is_jumping(mech)) {
          char empty_command[] = "";
          mech_land(mech_pilot_dbref(mech), mech, empty_command);
        }
        char empty_command[] = "";
        mech_shutdown(mech_pilot_dbref(mech), mech, empty_command);
        (void)snprintf(message_buffer, MBUF_SIZE,
                       "move_mech:invalid map:Mech: %ld Index: %ld",
                       mech_dbref(mech), mech_map_dbref(mech));
        btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                           message_buffer);
        mech_map_dbref_set(mech, -1);
      }
      return;
    }

    if (mech_condition_summary(mech).hidden) {
      mech_notify(mech, MECHALL, "You move too much and break your cover!");
      mech_los_broadcast(mech, "breaks from its cover.");
      mech_hidden_set(mech, false);
    }
    mech_event_cancel(mech, EVENT_HIDE);

    const int X = mech_position_x(mech);
    const int Y = mech_position_y(mech);
    int iced = 0;
    if (step.update_surface) {
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE) {
        if (step.previous_z < -1 && mech_position_z(mech) >= -1)
          break_thru_ice(mech);
        else if (mech_position_z(mech) == 0 && possibly_drop_thru_ice(mech))
          iced = 1;
      }

      mech_drop_surface_set(mech, false);
      if (mech_class(mech) == CLASS_MECH &&
          mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
          step.previous_z == -1 && mech_position_z(mech) == -1) {
        mech_position_z_set(mech, 0);
      }
    }

    if (!iced)
      mech_hex_entry_resolve(&(MechHexEntryRequest){
          .mech = mech,
          .map = mech_map,
          .delta = {.x = step.delta_x, .y = step.delta_y},
          .previous_z = last_z,
      });

    if (mech_position_x(mech) == X && mech_position_y(mech) == Y) {
      mech_flood(mech);
      mech_inferno_extinguish_in_water(mech);
      steppable_base_check(mech, X, Y);

      if (is_in_character(btech_context_database(mech_context(mech)),
                          mech_dbref(mech))) {
        int hexes_walked = mech_hexes_walked_advance(mech);
        if (!(hexes_walked % PIL_XP_EVERY_N_STEPS) &&
            mech_has_active_pilot(mech))
          piloting_experience_award(&(PilotingExperienceAward){
              .pilot = mech_pilot_dbref(mech),
              .mech = mech,
              .reason = 1,
              .unconditional = false,
          });
      }

      mech_domino_resolve(mech, MECH_DOMINO_GROUND);
    }
  }

  if ((mech_movement_type(mech) == MOVE_VTOL || mech_is_aerospace_unit(mech)) &&
      !mech_is_landed(mech))
    mech_vtol_altitude_check(mech);
  if (mech_class(mech) == CLASS_VEH_NAVAL)
    mech_naval_altitude_check(mech, step.previous_z);

  mech_charge_impact_resolve(mech);
  mech_towing_position_update(mech);
  bsuit_swarmers_position_update(mech_map, mech);
  mech_fire_hazard_resolve(mech);
}
