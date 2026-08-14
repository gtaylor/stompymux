#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "environment_damage_api.h"
#include "equipment_types.h"
#include "mech_advanced_api.h"
#include "mech_ammunition_explosion_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"

static void mech_self_destruct_event(MuxEvent *event) {
  Mech *mech = event->data;
  long extra = (long)event->data2;

  if (mech_is_destroyed(mech) || !mech_is_started(mech))
    return;

  if (extra > 256 && !destructive_ammunition_find(mech).damage)
    return;

  if ((--extra) % 256) {
    mech_printf(mech, MECHALL, "Self-destruction in %ld second%s..",
                extra % 256, extra > 1 ? "s" : "");
    mech_event_schedule(mech, EVENT_EXPLODE, mech_self_destruct_event, 1,
                        extra);
    return;
  }

  BtechContext *context = mech_context(mech);
  btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "#%ld explodes.",
                     mech_dbref(mech));
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_notify(mech, MECHALL,
                "Your batttle suit triggers it's self-destruction sequence.. "
                "you faint.. (and die)");
    mech_los_broadcast(mech, "suddenly explodes!");
    headhitmwdamage(mech, mech, 4);
    for (int section = 0; section < NUM_BSUIT_MEMBERS; section++)
      mech_section_destroy(&(SectionDestructionRequest){.wounded = mech,
                                                        .attacker = mech,
                                                        .line_of_sight = -1,
                                                        .section = section});
    mech_position_z_set(mech, mech_position_z(mech) + 6);
  } else if (mech_class(mech) != CLASS_MECH) {
    mech_notify(mech, MECHALL,
                "Your life flashes before your eyes as your vehicle "
                "immolates itself... you faint.. (and die)");
    mech_los_broadcast(mech, "suddenly explodes!");
    mech_section_destroy(&(SectionDestructionRequest){
        .wounded = mech, .attacker = mech, .line_of_sight = -1, .section = 3});
    headhitmwdamage(mech, mech, 4);
    mech_position_z_set(mech, mech_position_z(mech) + 6);
  } else if (extra >= 256) {
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld explodes [ammo]", mech_dbref(mech));
    mech_notify(mech, MECHALL, "All your ammo explodes!");
    for (;;) {
      AmmunitionHazardResult ammunition = destructive_ammunition_find(mech);
      if (!ammunition.damage)
        break;
      mech_ammunition_explode(
          &(AmmunitionExplosionRequest){.attacker = mech,
                                        .target = mech,
                                        .ammunition = ammunition.slot,
                                        .damage = ammunition.damage});
    }
  } else {
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld explodes [reactor]", mech_dbref(mech));
    mech_los_broadcast(mech, "suddenly explodes!");
    mech_notify(mech, MECHALL,
                "Suddenly you feel great heat overcoming your senses.. you "
                "faint.. (and die)");
    mech_reactor_explode(mech, mech);
  }
}

void mech_explode(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  char *args[3];
  long time = btech_context_self_destruct_time(context);
  bool ammunition = true;
  bool override;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  override = strstr(buffer, "override") != nullptr &&
             is_wizard(btech_context_database(context), player);
  int argument_count = mech_parseattributes(buffer, args, 2);
  if (argument_count < 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return;
  }

  if (!override) {
    for (int section = 0; section < NUM_SECTIONS; section++) {
      if (!mech_section_is_destroyed(mech, section)) {
        if (mech_section_has_recycling_weapon(mech, section)) {
          mecha_notify(btech_context_evaluation(context), player,
                       "You have weapons recycling!");
          return;
        }
      }
      if (mech_section_recycle_ticks(mech, section)) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You are still recovering from your last attack.");
        return;
      }
    }
  }

  if (!strcasecmp(buffer, "stop")) {
    if (!override) {
      if (!btech_context_self_destruct_can_stop(context)) {
        mecha_notify(btech_context_evaluation(context), player,
                     "It's too late to turn back now!");
        return;
      }
    }
    if (!mech_event_count(mech, EVENT_EXPLODE)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Your mech isn't undergoing a self-destruct sequence!");
      return;
    }

    mech_event_cancel(mech, EVENT_EXPLODE);
    mech_notify(mech, MECHALL, "Self-destruction sequence aborted.");
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld in #%ld stopped the self-destruction sequence.",
                       player, mech_dbref(mech));
    mech_los_broadcast(mech, "regains control over itself.");
    return;
  }

  if (mech_event_count(mech, EVENT_EXPLODE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your mech is already undergoing a self-destruct sequence!");
    return;
  }
  if (!strcasecmp(buffer, "ammo")) {
    if (!override) {
      if (!btech_context_self_destruct_ammunition_enabled(context)) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You can't bring yourself to do it!");
        return;
      }
      if (mech_condition_summary(mech).self_destruct_safe) {
        mecha_notify(btech_context_evaluation(context), player,
                     "That's not a possibility here.");
        return;
      }
    }
    if (!destructive_ammunition_find(mech).damage) {
      mecha_notify(btech_context_evaluation(context), player,
                   "There is no 'damaging' ammo on your 'mech!");
      return;
    }
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld in #%ld initiates the ammo explosion sequence.",
                       player, mech_dbref(mech));
    mech_los_broadcast(mech, "starts billowing smoke!");
    time /= 2;
  } else {
    if (!override) {
      if (!btech_context_self_destruct_reactor_enabled(context)) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You can't bring yourself to do it!");
        return;
      }
      if (mech_class(mech) != CLASS_MECH) {
        mecha_notify(btech_context_evaluation(context), player,
                     "Only mechs can do the 'big boom' effect.");
        return;
      }
      if (mech_technology_flags(mech) & ICE_TECH) {
        mecha_notify(btech_context_evaluation(context), player,
                     "You need a fusion reactor.");
        return;
      }
    }
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG,
                       "#%ld in #%ld initiates the reactor explosion sequence.",
                       player, mech_dbref(mech));
    mech_los_broadcast(mech, "loses reactions containment!");
    ammunition = false;
  }
  if (override)
    time = 3;
  mech_event_schedule(mech, EVENT_EXPLODE, mech_self_destruct_event, 1, time);
  mech_notify(mech, MECHALL,
              "Self-destruction sequence engaged ; please stand by.");
  mech_printf(mech, MECHALL, "%s in %ld seconds.",
              ammunition ? "The ammunition will explode"
                         : "The reactor will blow up",
              time);
  mech_pilot_dbref_set(mech, -1);
}
