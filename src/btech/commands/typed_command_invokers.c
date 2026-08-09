#include "aero_move_api.h"
#include "ai_api.h"
#include "command_invokers.h"
#include "command_registry.h"
#include "map_conditions_api.h"
#include "mech_advanced_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_fire_api.h"
#include "mech_ice_api.h"
#include "mech_tech_repairs_api.h"
#include "registry_api.h"

#define DEFINE_MECH_COMMAND_INVOKER(invoker, handler)                          \
  void btech_command_invoke_##invoker(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(                                                                   \
        invocation->actor,                                                     \
        btech_context_get_mech(invocation->context, invocation->object_id),    \
        invocation->arguments);                                                \
  }

DEFINE_MECH_COMMAND_INVOKER(aero_climb, aero_climb)
DEFINE_MECH_COMMAND_INVOKER(aero_dive, aero_dive)
DEFINE_MECH_COMMAND_INVOKER(mech_stinger, mech_stinger)
DEFINE_MECH_COMMAND_INVOKER(mech_usebin, mech_usebin)
DEFINE_MECH_COMMAND_INVOKER(mech_weapon_status, mech_weapon_status)
DEFINE_MECH_COMMAND_INVOKER(aero_checklz, aero_checklz)
DEFINE_MECH_COMMAND_INVOKER(mech_damage, mech_damage)
DEFINE_MECH_COMMAND_INVOKER(mech_damage_section, mech_damage_section)
DEFINE_MECH_COMMAND_INVOKER(mech_ecm, mech_ecm)
DEFINE_MECH_COMMAND_INVOKER(mech_eccm, mech_eccm)
DEFINE_MECH_COMMAND_INVOKER(mech_angelecm, mech_angelecm)
DEFINE_MECH_COMMAND_INVOKER(mech_angeleccm, mech_angeleccm)
DEFINE_MECH_COMMAND_INVOKER(mech_perecm, mech_perecm)
DEFINE_MECH_COMMAND_INVOKER(mech_pereccm, mech_pereccm)
DEFINE_MECH_COMMAND_INVOKER(mech_stealtharmor, mech_stealtharmor)
DEFINE_MECH_COMMAND_INVOKER(mech_nullsig, mech_nullsig)
DEFINE_MECH_COMMAND_INVOKER(mech_slite, mech_slite)
DEFINE_MECH_COMMAND_INVOKER(mech_auto_turret, mech_auto_turret)
DEFINE_MECH_COMMAND_INVOKER(vehicle_fire_extinguish, vehicle_fire_extinguish)
DEFINE_MECH_COMMAND_INVOKER(mech_c3_message, mech_c3_message)
DEFINE_MECH_COMMAND_INVOKER(mech_c3_targets, mech_c3_targets)
DEFINE_MECH_COMMAND_INVOKER(mech_c3_network, mech_c3_network)
DEFINE_MECH_COMMAND_INVOKER(mech_c3i_message, mech_c3i_message)
DEFINE_MECH_COMMAND_INVOKER(mech_c3i_targets, mech_c3i_targets)
DEFINE_MECH_COMMAND_INVOKER(mech_c3i_network, mech_c3i_network)
DEFINE_MECH_COMMAND_INVOKER(show_narc_pods, show_narc_pods)
DEFINE_MECH_COMMAND_INVOKER(remove_inarc_pods_mech, remove_inarc_pods_mech)
DEFINE_MECH_COMMAND_INVOKER(remove_inarc_pods_tank, remove_inarc_pods_tank)
DEFINE_MECH_COMMAND_INVOKER(tech_repairs, tech_repairs)
DEFINE_MECH_COMMAND_INVOKER(mech_snipe, mech_snipe)

#undef DEFINE_MECH_COMMAND_INVOKER

#define DEFINE_MAP_COMMAND_INVOKER(handler)                                    \
  void btech_command_invoke_##handler(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(invocation->actor,                                                 \
            btech_context_get_map(invocation->context, invocation->object_id), \
            invocation->arguments);                                            \
  }

DEFINE_MAP_COMMAND_INVOKER(map_addice)
DEFINE_MAP_COMMAND_INVOKER(map_delice)
DEFINE_MAP_COMMAND_INVOKER(map_setconditions)

#undef DEFINE_MAP_COMMAND_INVOKER
