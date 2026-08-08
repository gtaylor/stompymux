#include "command_invokers.h"

#include "autopilot.h"

#define DEFINE_BTECH_COMMAND_INVOKER(handler)                                  \
  void handler(DbRef actor, void *object, char *arguments);                    \
  void btech_command_invoke_##handler(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(invocation->actor, invocation->object, invocation->arguments);     \
  }
DEFINE_BTECH_COMMAND_INVOKER(ListForms)
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
DEFINE_BTECH_COMMAND_INVOKER(debug_EventTypes)
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
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Raddinftech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Raddspecial)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Raddtech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Raddweap)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rdelinftech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rdeltech)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rdisplaysection)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rfiremode)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rloadnew)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rreload)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rrepair)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rresetcrits)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rrestock)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rrestore)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsavetemp)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsavetemp2)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetarmor)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetheatsinks)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetjumpspeed)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetlrsrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetmove)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetradio)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetradiorange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetscanrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsetspeed)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsettacrange)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsettarget)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsettons)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rsettype)
DEFINE_BTECH_COMMAND_INVOKER(mechrep_Rshowtech)
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
