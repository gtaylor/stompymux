#include "command_invokers.h"

#include "autopilot.h"
#include "command_registry.h"
#include "mux/server/platform.h"

#define DEFINE_BTECH_COMMAND_INVOKER(handler)                                  \
  void handler(DbRef actor, void *object, char *arguments);                    \
  void btech_command_invoke_##handler(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(invocation->actor, invocation->object, invocation->arguments);     \
  }
DEFINE_BTECH_COMMAND_INVOKER(list_forms)
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
DEFINE_BTECH_COMMAND_INVOKER(mechrep_raddinftech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_raddspecial)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_raddtech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_raddweap)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rdelinftech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rdeltech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rdisplaysection)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rfiremode)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rloadnew)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rreload)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rrepair)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rresetcrits)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rrestock)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rrestore)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsavetemp)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsavetemp2)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetarmor)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetheatsinks)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetjumpspeed)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetlrsrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetmove)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetradio)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetradiorange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetscanrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsetspeed)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsettacrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsettarget)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsettons)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rsettype)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_rshowtech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_setcargospace)
DEFINE_BTECH_COMMAND_INVOKER(mine_command_add)
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
