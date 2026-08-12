#include "mech_ammunition_explosion_api.h"
#include "mech_update_api.h"

#include <math.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "registry_api.h"

static int mech_jump_speed_mp(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (mech_is_under_gravity(mech) && map) {
    int gravity = battle_map_gravity(map);
    speed = speed * 100.0F / (float)(gravity > 50 ? gravity : 50);
  }
  return (int)(speed * MP_PER_KPH);
}

void mech_overheat_handle(Mech *mech) {
  int avoided = 0;
  int hasinferno = 0;
  BattleMap *mech_map;
  AmmunitionHazardResult ammunition = {0};
  BtechContext *context = mech_context(mech);
  float heat = mech_excess_heat(mech);
  int tick = btech_context_event_tick(context);

  if (heat < 10.0F)
    return;
  if ((mech_last_overheat_check_tick(mech) + TURN) > tick)
    return;
  mech_last_overheat_check_tick_set(mech, tick);

  if (heat >= 10.0F) {
    if (btech_context_inferno_penalty_enabled(context)) {
      ammunition = inferno_ammunition_find(mech);
    }
    hasinferno = ammunition.damage;
    if (heat >= 28.0F) {
      if (hasinferno) {
        if (btech_random_roll(context) >= 12)
          avoided = 1;
      } else if (btech_random_roll(context) >= 8) {
        avoided = 1;
      }
    } else if (heat >= 23.0F) {
      if (hasinferno) {
        if (btech_random_roll(context) >= 10)
          avoided = 1;
      } else if (btech_random_roll(context) >= 6) {
        avoided = 1;
      }
    } else if (heat >= 19.0F) {
      if (hasinferno) {
        if (btech_random_roll(context) >= 8)
          avoided = 1;
      } else if (btech_random_roll(context) >= 4) {
        avoided = 1;
      }
    } else if ((heat >= 14.0F) && hasinferno) {
      if (btech_random_roll(context) >= 6)
        avoided = 1;
    } else if ((heat >= 10.0F) && hasinferno) {
      if (btech_random_roll(context) >= 4)
        avoided = 1;
    } else if ((heat < 19.0F) && !hasinferno) {
      avoided = 1;
    }

    if (!avoided) {
      if (!hasinferno)
        ammunition = destructive_ammunition_find(mech);
      if (ammunition.damage) {
        mech_ammunition_explode(
            &(AmmunitionExplosionRequest){.attacker = mech,
                                          .target = mech,
                                          .ammunition = ammunition.slot,
                                          .damage = ammunition.damage});
      } else {
        mech_notify(mech, MECHALL, "You have no ammunition, lucky you!");
      }
    }
  }

  avoided = 0;
#ifdef BT_EXILE_MW3STATS
  if (!is_player(btech_context_database(context), mech_pilot_dbref(mech))) {
#endif
    if (heat >= 30.0F) {
    } else if (heat >= 26.0F) {
      if (btech_random_roll(context) >= 10)
        avoided = 1;
    } else if (heat >= 22.0F) {
      if (btech_random_roll(context) >= 8)
        avoided = 1;
    } else if (heat >= 18.0F) {
      if (btech_random_roll(context) >= 6)
        avoided = 1;
    } else if (heat >= 14.0F) {
      if (btech_random_roll(context) >= 4)
        avoided = 1;
    }
#ifdef BT_EXILE_MW3STATS
  } else {
    avoided = 1;
    if (heat >= 14.0F) {
      mech_notify(mech, MECHALL,
                  "You frantically attempt to override the shutdown process!");
      avoided = char_getskillsuccess(
          &(CharacterSkillCheck){.context = context,
                                 .player = mech_pilot_dbref(mech),
                                 .name = "computer",
                                 .modifier = (heat >= 30.0F   ? 8
                                              : heat >= 26.0F ? 6
                                              : heat >= 22.0F ? 4
                                              : heat >= 18.0F ? 2
                                                              : 0),
                                 .loud = true});
      if (avoided)
        accumulate_computer_xp(mech_pilot_dbref(mech), mech, 1);
    }
  }
#endif
  if (!avoided && mech_is_started(mech)) {
    mech_notify(mech, MECHALL,
                "[fg=red inverse]Reactor shutting down...[reset]");
    if (mech_searchlight_active(mech)) {
      mech_notify(mech, MECHALL, "Your searchlight shuts off.");
      mech_searchlight_active_set(mech, false);
      mech_illumination_set(mech, false);
    }
    if (mech_is_jumping(mech) || mech_is_out_of_control(mech) ||
        (mech_is_aerospace_unit(mech) && !mech_is_landed(mech))) {
      mech_notify(mech, MECHALL, "[bold]You fall from the sky![reset]");
      mech_los_broadcast(mech, "falls from the sky!");
      mech_map = btech_context_get_map(context, mech_map_dbref(mech));
      mech_fall(mech, mech_jump_speed_mp(mech, mech_map), 0);
      mech_domino_resolve(mech, MECH_DOMINO_FALL);
    } else {
      mech_los_broadcast(mech, "stops in mid-motion!");
      if ((fabsf(mech_current_speed(mech)) > MP1) && !mech_is_fallen(mech) &&
          !made_pilot_skill_roll(mech, 3))
        mech_fall(mech, 0, 1);
    }
    mech_power_down(mech);
    mech_event_cancel(mech, EVENT_MOVE);
    mech_event_cancel(mech, EVENT_STAND);
  }
}
