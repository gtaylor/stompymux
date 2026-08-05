#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "environment_damage_api.h"
#include "legacy_macros.h"
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
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "section_types.h"

static void mech_self_destruct_event(MuxEvent *event) {
  Mech *mech = event->data;
  long extra = (long)event->data2;
  int ammunition_section;
  int ammunition_critical;
  int damage;

  if (mech_is_destroyed(mech) || !mech_is_started(mech))
    return;

  if (extra > 256 &&
      !FindDestructiveAmmo(mech, &ammunition_section, &ammunition_critical))
    return;

  if ((--extra) % 256) {
    mech_printf(mech, MECHALL, "Self-destruction in %ld second%s..",
                extra % 256, extra > 1 ? "s" : "");
    mech_event_schedule(mech, EVENT_EXPLODE, mech_self_destruct_event, 1,
                        extra);
    return;
  }

  BtechContext *context = mech_context(mech);
  btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("#%ld explodes.", mech_dbref(mech)));
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_notify(mech, MECHALL,
                "Your batttle suit triggers it's self-destruction sequence.. "
                "you faint.. (and die)");
    mech_los_broadcast(mech, "suddenly explodes!");
    headhitmwdamage(mech, mech, 4);
    for (int section = 0; section < NUM_BSUIT_MEMBERS; section++)
      DestroySection(mech, mech, -1, section);
    mech_position_z_set(mech, mech_position_z(mech) + 6);
  } else if (mech_class(mech) != CLASS_MECH) {
    mech_notify(mech, MECHALL,
                "Your life flashes before your eyes as your vehicle "
                "immolates itself... you faint.. (and die)");
    mech_los_broadcast(mech, "suddenly explodes!");
    DestroySection(mech, mech, -1, 3);
    headhitmwdamage(mech, mech, 4);
    mech_position_z_set(mech, mech_position_z(mech) + 6);
  } else if (extra >= 256) {
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("#%ld explodes [ammo]", mech_dbref(mech)));
    mech_notify(mech, MECHALL, "All your ammo explodes!");
    while ((damage = FindDestructiveAmmo(mech, &ammunition_section,
                                         &ammunition_critical)))
      mech_ammunition_explode(mech, mech, ammunition_section,
                              ammunition_critical, damage);
  } else {
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("#%ld explodes [reactor]", mech_dbref(mech)));
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
  int ammunition_section;
  int ammunition_critical;
  long time = btech_context_self_destruct_time(context);
  bool ammunition = true;
  bool override;

  cch(MECH_USUALO);
  override = strstr(buffer, "override") != nullptr &&
             is_wizard(btech_context_database(context), player);
  int argument_count = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(context, argument_count < 1, "Invalid number of arguments!");

  if (!override) {
    for (int section = 0; section < NUM_SECTIONS; section++) {
      if (!mech_section_is_destroyed(mech, section))
        DOCHECK_CONTEXT(context,
                        mech_section_has_recycling_weapon(mech, section),
                        "You have weapons recycling!");
      DOCHECK_CONTEXT(context, mech_section_recycle_ticks(mech, section),
                      "You are still recovering from your last attack.");
    }
  }

  if (!strcasecmp(buffer, "stop")) {
    if (!override)
      DOCHECK_CONTEXT(context, !btech_context_self_destruct_can_stop(context),
                      "It's too late to turn back now!");
    DOCHECK_CONTEXT(context, !mech_event_count(mech, EVENT_EXPLODE),
                    "Your mech isn't undergoing a self-destruct sequence!");

    mech_event_cancel(mech, EVENT_EXPLODE);
    mech_notify(mech, MECHALL, "Self-destruction sequence aborted.");
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld stopped the self-destruction sequence.", player,
                mech_dbref(mech)));
    mech_los_broadcast(mech, "regains control over itself.");
    return;
  }

  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_EXPLODE),
                  "Your mech is already undergoing a self-destruct sequence!");
  if (!strcasecmp(buffer, "ammo")) {
    if (!override) {
      DOCHECK_CONTEXT(context,
                      !btech_context_self_destruct_ammunition_enabled(context),
                      "You can't bring yourself to do it!");
      DOCHECK_CONTEXT(context, mech_condition_summary(mech).self_destruct_safe,
                      "That's not a possibility here.");
    }
    int damage =
        FindDestructiveAmmo(mech, &ammunition_section, &ammunition_critical);
    DOCHECK_CONTEXT(context, !damage,
                    "There is no 'damaging' ammo on your 'mech!");
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld initiates the ammo explosion sequence.", player,
                mech_dbref(mech)));
    mech_los_broadcast(mech, "starts billowing smoke!");
    time /= 2;
  } else {
    if (!override) {
      DOCHECK_CONTEXT(context,
                      !btech_context_self_destruct_reactor_enabled(context),
                      "You can't bring yourself to do it!");
      DOCHECK_CONTEXT(context, mech_class(mech) != CLASS_MECH,
                      "Only mechs can do the 'big boom' effect.");
      DOCHECK_CONTEXT(context, mech_technology_flags(mech) & ICE_TECH,
                      "You need a fusion reactor.");
    }
    btech_channel_send(
        context, BTECH_CHANNEL_MECH_DEBUG, "%s",
        tprintf("#%ld in #%ld initiates the reactor explosion sequence.",
                player, mech_dbref(mech)));
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
