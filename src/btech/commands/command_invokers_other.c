#include "command_invokers.h"

#include "autopilot.h"
#include "command_registry.h"
#include "econ_cmds_api.h"
#include "mux/server/platform.h"
#include "value_handlers_api.h"

#define DEFINE_BTECH_COMMAND_INVOKER(handler)                                  \
  void handler(DbRef player, void *data, char *buffer);                        \
  void btech_command_invoke_##handler(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(invocation->actor, invocation->object, invocation->arguments);     \
  }
DEFINE_BTECH_COMMAND_INVOKER(list_forms)
void btech_command_invoke_list_special_values(
    const BtechCommandInvocation *invocation) {
  list_special_values(invocation->actor, invocation->object,
                      invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(auto_addcommand)
void btech_command_invoke_auto_delcommand(
    const BtechCommandInvocation *invocation) {
  auto_delcommand(invocation->actor, invocation->object, invocation->arguments);
}
void btech_command_invoke_auto_disengage(
    const BtechCommandInvocation *invocation) {
  auto_disengage(invocation->actor, invocation->object, invocation->arguments);
}
void btech_command_invoke_auto_engage(
    const BtechCommandInvocation *invocation) {
  auto_engage(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(auto_eventstats)
DEFINE_BTECH_COMMAND_INVOKER(auto_jump)
DEFINE_BTECH_COMMAND_INVOKER(auto_listcommands)
DEFINE_BTECH_COMMAND_INVOKER(debug_event_types)
DEFINE_BTECH_COMMAND_INVOKER(debug_fixmap)
DEFINE_BTECH_COMMAND_INVOKER(debug_memory)
DEFINE_BTECH_COMMAND_INVOKER(debug_savedb)
DEFINE_BTECH_COMMAND_INVOKER(debug_setvrt)
DEFINE_BTECH_COMMAND_INVOKER(debug_setwbv)
DEFINE_BTECH_COMMAND_INVOKER(debug_setxplevel)
DEFINE_BTECH_COMMAND_INVOKER(debug_shutdown)
DEFINE_BTECH_COMMAND_INVOKER(debug_xptop)
DEFINE_BTECH_COMMAND_INVOKER(map_add_block)
DEFINE_BTECH_COMMAND_INVOKER(map_addfire)
DEFINE_BTECH_COMMAND_INVOKER(map_addhex)
DEFINE_BTECH_COMMAND_INVOKER(map_addsmoke)
DEFINE_BTECH_COMMAND_INVOKER(map_clearmechs)
DEFINE_BTECH_COMMAND_INVOKER(map_delobj)
DEFINE_BTECH_COMMAND_INVOKER(map_listmechs)
DEFINE_BTECH_COMMAND_INVOKER(map_loadmap)
DEFINE_BTECH_COMMAND_INVOKER(map_mapemit)
DEFINE_BTECH_COMMAND_INVOKER(map_savemap)
DEFINE_BTECH_COMMAND_INVOKER(map_setlinked)
DEFINE_BTECH_COMMAND_INVOKER(map_setmapsize)
DEFINE_BTECH_COMMAND_INVOKER(map_updatelinks)
DEFINE_BTECH_COMMAND_INVOKER(map_view)
void btech_command_invoke_mech_manifest(
    const BtechCommandInvocation *invocation) {
  mech_manifest(invocation->actor, invocation->object, invocation->arguments);
}
void btech_command_invoke_mech_raddstuff(
    const BtechCommandInvocation *invocation) {
  mech_raddstuff(invocation->actor, invocation->object, invocation->arguments);
}
void btech_command_invoke_mech_rfixstuff(
    const BtechCommandInvocation *invocation) {
  mech_rfixstuff(invocation->actor, invocation->object, invocation->arguments);
}
void btech_command_invoke_mech_rremovestuff(
    const BtechCommandInvocation *invocation) {
  mech_rremovestuff(invocation->actor, invocation->object,
                    invocation->arguments);
}
void btech_command_invoke_mech_rresetstuff(
    const BtechCommandInvocation *invocation) {
  mech_rresetstuff(invocation->actor, invocation->object,
                   invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(mine_command_add)
void btech_command_invoke_set_special_value(
    const BtechCommandInvocation *invocation) {
  set_special_value(invocation->actor, invocation->object,
                    invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(turret_addtic)
DEFINE_BTECH_COMMAND_INVOKER(turret_bearing)
DEFINE_BTECH_COMMAND_INVOKER(turret_cleartic)
DEFINE_BTECH_COMMAND_INVOKER(turret_contacts)
DEFINE_BTECH_COMMAND_INVOKER(turret_critstatus)
DEFINE_BTECH_COMMAND_INVOKER(turret_deinitialize)
DEFINE_BTECH_COMMAND_INVOKER(turret_deltic)
DEFINE_BTECH_COMMAND_INVOKER(turret_eta)
DEFINE_BTECH_COMMAND_INVOKER(turret_findcenter)
DEFINE_BTECH_COMMAND_INVOKER(turret_firetic)
DEFINE_BTECH_COMMAND_INVOKER(turret_fireweapon)
DEFINE_BTECH_COMMAND_INVOKER(turret_initialize)
DEFINE_BTECH_COMMAND_INVOKER(turret_listtic)
DEFINE_BTECH_COMMAND_INVOKER(turret_lrsmap)
DEFINE_BTECH_COMMAND_INVOKER(turret_navigate)
DEFINE_BTECH_COMMAND_INVOKER(turret_range)
DEFINE_BTECH_COMMAND_INVOKER(turret_report)
DEFINE_BTECH_COMMAND_INVOKER(turret_scan)
DEFINE_BTECH_COMMAND_INVOKER(turret_settarget)
DEFINE_BTECH_COMMAND_INVOKER(turret_sight)
DEFINE_BTECH_COMMAND_INVOKER(turret_status)
DEFINE_BTECH_COMMAND_INVOKER(turret_tacmap)
DEFINE_BTECH_COMMAND_INVOKER(turret_weaponspecs)

#undef DEFINE_BTECH_COMMAND_INVOKER
