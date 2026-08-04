#pragma once

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "environment_damage_api.h"
#include "failures.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_build_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "random.h"

#define SILLY_TOGGLE_MACRO(neededspecial, setstatus, msgon, msgoff, donthave)  \
  if (MechSpecials(mech) & (neededspecial)) {                                  \
    if (MechStatus(mech) & (setstatus)) {                                      \
      mech_notify(mech, MECHALL, msgoff);                                      \
      MechStatus(mech) &= ~(setstatus);                                        \
    } else {                                                                   \
      mech_notify(mech, MECHALL, msgon);                                       \
      MechStatus(mech) |= (setstatus);                                         \
    }                                                                          \
  } else                                                                       \
    notify(btech_context_evaluation(mech->xcode.context), player, donthave)

#define TOGGLE_SPECIALS_MACRO_CHECK(neededspecial, setstatus, offstatus,       \
                                    msgon, msgoff, donthave)                   \
  if (MechSpecials(mech) & (neededspecial)) {                                  \
    if (MechStatus2(mech) & (setstatus)) {                                     \
      mech_notify(mech, MECHALL, msgoff);                                      \
      MechStatus2(mech) &= ~(setstatus);                                       \
    } else {                                                                   \
      mech_notify(mech, MECHALL, msgon);                                       \
      MechStatus2(mech) |= (setstatus);                                        \
      MechStatus2(mech) &= ~(offstatus);                                       \
    }                                                                          \
  } else                                                                       \
    notify(btech_context_evaluation(mech->xcode.context), player, donthave)

#define TOGGLE_SPECIALS_MACRO_CHECK2(neededspecial, setstatus, offstatus,      \
                                     msgon, msgoff, donthave)                  \
  if (MechSpecials2(mech) & (neededspecial)) {                                 \
    if (MechStatus2(mech) & (setstatus)) {                                     \
      mech_notify(mech, MECHALL, msgoff);                                      \
      MechStatus2(mech) &= ~(setstatus);                                       \
    } else {                                                                   \
      mech_notify(mech, MECHALL, msgon);                                       \
      MechStatus2(mech) |= (setstatus);                                        \
      MechStatus2(mech) &= ~(offstatus);                                       \
    }                                                                          \
  } else                                                                       \
    notify(btech_context_evaluation(mech->xcode.context), player, donthave)

#define TOGGLE_INFANTRY_MACRO_CHECK(neededspecial, setstatus, offstatus,       \
                                    msgon, msgoff, donthave)                   \
  if (MechInfantrySpecials(mech) & (neededspecial)) {                          \
    if (MechStatus2(mech) & (setstatus)) {                                     \
      mech_notify(mech, MECHALL, msgoff);                                      \
      MechStatus2(mech) &= ~(setstatus);                                       \
    } else {                                                                   \
      mech_notify(mech, MECHALL, msgon);                                       \
      MechStatus2(mech) |= (setstatus);                                        \
      MechStatus2(mech) &= ~(offstatus);                                       \
    }                                                                          \
  } else                                                                       \
    notify(btech_context_evaluation(mech->xcode.context), player, donthave)
