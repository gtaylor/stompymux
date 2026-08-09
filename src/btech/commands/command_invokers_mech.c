#include "command_invokers.h"

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "command_registry.h"
#include "eject_api.h"
#include "mech_move_api.h"
#include "mech_startup_api.h"
#include "mux/server/platform.h"

#define DEFINE_BTECH_COMMAND_INVOKER(handler)                                  \
  void handler(DbRef actor, void *object, char *arguments);                    \
  void btech_command_invoke_##handler(                                         \
      const BtechCommandInvocation *invocation) {                              \
    handler(invocation->actor, invocation->object, invocation->arguments);     \
  }
void btech_command_invoke_aero_takeoff(
    const BtechCommandInvocation *invocation) {
  aero_takeoff(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(aero_thrust)
DEFINE_BTECH_COMMAND_INVOKER(bsuit_attackleg)
void btech_command_invoke_bsuit_hide(const BtechCommandInvocation *invocation) {
  bsuit_hide(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(bsuit_pack_jettison)
DEFINE_BTECH_COMMAND_INVOKER(bsuit_swarm)
DEFINE_BTECH_COMMAND_INVOKER(heat_cutoff)
DEFINE_BTECH_COMMAND_INVOKER(list_xcodestuff)
DEFINE_BTECH_COMMAND_INVOKER(mech_Raddstuff)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rfixstuff)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rremovestuff)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rresetstuff)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rsetmapindex)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rsetteam)
DEFINE_BTECH_COMMAND_INVOKER(mech_Rsetxy)
DEFINE_BTECH_COMMAND_INVOKER(mech_addtic)
DEFINE_BTECH_COMMAND_INVOKER(mech_ams)
DEFINE_BTECH_COMMAND_INVOKER(mech_armorpiercing)
DEFINE_BTECH_COMMAND_INVOKER(mech_artemis)
DEFINE_BTECH_COMMAND_INVOKER(mech_atmexplosive)
DEFINE_BTECH_COMMAND_INVOKER(mech_atmrange)
DEFINE_BTECH_COMMAND_INVOKER(mech_attachcables)
DEFINE_BTECH_COMMAND_INVOKER(mech_axe)
DEFINE_BTECH_COMMAND_INVOKER(mech_bearing)
DEFINE_BTECH_COMMAND_INVOKER(mech_bomb)
DEFINE_BTECH_COMMAND_INVOKER(mech_bootlegger)
DEFINE_BTECH_COMMAND_INVOKER(mech_brief)
DEFINE_BTECH_COMMAND_INVOKER(mech_c3_join_leave)
DEFINE_BTECH_COMMAND_INVOKER(mech_c3i_join_leave)
DEFINE_BTECH_COMMAND_INVOKER(mech_caseless)
DEFINE_BTECH_COMMAND_INVOKER(mech_charge)
DEFINE_BTECH_COMMAND_INVOKER(mech_claw)
DEFINE_BTECH_COMMAND_INVOKER(mech_cleartic)
DEFINE_BTECH_COMMAND_INVOKER(mech_club)
DEFINE_BTECH_COMMAND_INVOKER(mech_cluster)
DEFINE_BTECH_COMMAND_INVOKER(mech_contacts)
DEFINE_BTECH_COMMAND_INVOKER(mech_createbays)
DEFINE_BTECH_COMMAND_INVOKER(mech_critstatus)
DEFINE_BTECH_COMMAND_INVOKER(mech_deltic)
DEFINE_BTECH_COMMAND_INVOKER(mech_detachcables)
DEFINE_BTECH_COMMAND_INVOKER(mech_dig)
DEFINE_BTECH_COMMAND_INVOKER(mech_disableweap)
DEFINE_BTECH_COMMAND_INVOKER(mech_disembark)
DEFINE_BTECH_COMMAND_INVOKER(mech_dodge)
void btech_command_invoke_mech_drop(const BtechCommandInvocation *invocation) {
  mech_drop(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(mech_dropoff)
DEFINE_BTECH_COMMAND_INVOKER(mech_dump)
DEFINE_BTECH_COMMAND_INVOKER(mech_embark)
DEFINE_BTECH_COMMAND_INVOKER(mech_enterbase)
DEFINE_BTECH_COMMAND_INVOKER(mech_enterbay)
DEFINE_BTECH_COMMAND_INVOKER(mech_eta)
DEFINE_BTECH_COMMAND_INVOKER(mech_evade)
DEFINE_BTECH_COMMAND_INVOKER(mech_explode)
DEFINE_BTECH_COMMAND_INVOKER(mech_explosive)
DEFINE_BTECH_COMMAND_INVOKER(mech_findcenter)
DEFINE_BTECH_COMMAND_INVOKER(mech_firetic)
DEFINE_BTECH_COMMAND_INVOKER(mech_fireweapon)
DEFINE_BTECH_COMMAND_INVOKER(mech_fixturret)
DEFINE_BTECH_COMMAND_INVOKER(mech_flamerheat)
DEFINE_BTECH_COMMAND_INVOKER(mech_flechette)
DEFINE_BTECH_COMMAND_INVOKER(mech_fliparms)
DEFINE_BTECH_COMMAND_INVOKER(mech_gattling)
DEFINE_BTECH_COMMAND_INVOKER(mech_grabclub)
DEFINE_BTECH_COMMAND_INVOKER(mech_heading)
DEFINE_BTECH_COMMAND_INVOKER(mech_hotload)
DEFINE_BTECH_COMMAND_INVOKER(mech_hulldown)
DEFINE_BTECH_COMMAND_INVOKER(mech_inarc_ammo_toggle)
DEFINE_BTECH_COMMAND_INVOKER(mech_incendiary)
DEFINE_BTECH_COMMAND_INVOKER(mech_inferno)
DEFINE_BTECH_COMMAND_INVOKER(mech_jump)
DEFINE_BTECH_COMMAND_INVOKER(mech_kick)
DEFINE_BTECH_COMMAND_INVOKER(mech_land)
DEFINE_BTECH_COMMAND_INVOKER(mech_lateral)
DEFINE_BTECH_COMMAND_INVOKER(mech_lbx)
DEFINE_BTECH_COMMAND_INVOKER(mech_list_freqs)
DEFINE_BTECH_COMMAND_INVOKER(mech_listtic)
DEFINE_BTECH_COMMAND_INVOKER(mech_loadcargo)
DEFINE_BTECH_COMMAND_INVOKER(mech_losemit)
DEFINE_BTECH_COMMAND_INVOKER(mech_lrsmap)
DEFINE_BTECH_COMMAND_INVOKER(mech_mace)
DEFINE_BTECH_COMMAND_INVOKER(mech_manifest)
DEFINE_BTECH_COMMAND_INVOKER(mech_masc)
DEFINE_BTECH_COMMAND_INVOKER(mech_mechprefs)
DEFINE_BTECH_COMMAND_INVOKER(mech_mine)
DEFINE_BTECH_COMMAND_INVOKER(mech_narc)
DEFINE_BTECH_COMMAND_INVOKER(mech_navigate)
DEFINE_BTECH_COMMAND_INVOKER(mech_ood_initiate)
DEFINE_BTECH_COMMAND_INVOKER(mech_pickup)
DEFINE_BTECH_COMMAND_INVOKER(mech_precision)
DEFINE_BTECH_COMMAND_INVOKER(mech_punch)
DEFINE_BTECH_COMMAND_INVOKER(mech_rac)
DEFINE_BTECH_COMMAND_INVOKER(mech_radio)
DEFINE_BTECH_COMMAND_INVOKER(mech_range)
DEFINE_BTECH_COMMAND_INVOKER(mech_rapidfire)
DEFINE_BTECH_COMMAND_INVOKER(mech_report)
DEFINE_BTECH_COMMAND_INVOKER(mech_rotatetorso)
DEFINE_BTECH_COMMAND_INVOKER(mech_safety)
DEFINE_BTECH_COMMAND_INVOKER(mech_saw)
DEFINE_BTECH_COMMAND_INVOKER(mech_scan)
DEFINE_BTECH_COMMAND_INVOKER(mech_scharge)
DEFINE_BTECH_COMMAND_INVOKER(mech_sendchannel)
DEFINE_BTECH_COMMAND_INVOKER(mech_sensor)
DEFINE_BTECH_COMMAND_INVOKER(mech_set_channelfreq)
DEFINE_BTECH_COMMAND_INVOKER(mech_set_channelmode)
DEFINE_BTECH_COMMAND_INVOKER(mech_set_channeltitle)
DEFINE_BTECH_COMMAND_INVOKER(mech_set_target)
DEFINE_BTECH_COMMAND_INVOKER(mech_sguided)
void btech_command_invoke_mech_shutdown(
    const BtechCommandInvocation *invocation) {
  mech_shutdown(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(mech_sight)
DEFINE_BTECH_COMMAND_INVOKER(mech_smoke)
DEFINE_BTECH_COMMAND_INVOKER(mech_speed)
DEFINE_BTECH_COMMAND_INVOKER(mech_spot)
DEFINE_BTECH_COMMAND_INVOKER(mech_sprint)
DEFINE_BTECH_COMMAND_INVOKER(mech_stand)
void btech_command_invoke_mech_startup(
    const BtechCommandInvocation *invocation) {
  mech_startup(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(mech_status)
DEFINE_BTECH_COMMAND_INVOKER(mech_stores)
DEFINE_BTECH_COMMAND_INVOKER(mech_swarm)
DEFINE_BTECH_COMMAND_INVOKER(mech_swarm1)
DEFINE_BTECH_COMMAND_INVOKER(mech_sword)
DEFINE_BTECH_COMMAND_INVOKER(mech_tacmap)
DEFINE_BTECH_COMMAND_INVOKER(mech_tag)
DEFINE_BTECH_COMMAND_INVOKER(mech_target)
DEFINE_BTECH_COMMAND_INVOKER(mech_thrash)
DEFINE_BTECH_COMMAND_INVOKER(mech_trip)
DEFINE_BTECH_COMMAND_INVOKER(mech_turnmode)
DEFINE_BTECH_COMMAND_INVOKER(mech_turret)
void btech_command_invoke_mech_udisembark(
    const BtechCommandInvocation *invocation) {
  mech_udisembark(invocation->actor, invocation->object, invocation->arguments);
}
DEFINE_BTECH_COMMAND_INVOKER(mech_ultra)
DEFINE_BTECH_COMMAND_INVOKER(mech_unjamammo)
DEFINE_BTECH_COMMAND_INVOKER(mech_unloadcargo)
DEFINE_BTECH_COMMAND_INVOKER(mech_vector)
DEFINE_BTECH_COMMAND_INVOKER(mech_vertical)
DEFINE_BTECH_COMMAND_INVOKER(mech_view)
DEFINE_BTECH_COMMAND_INVOKER(mech_weaponspecs)
DEFINE_BTECH_COMMAND_INVOKER(mech_weight)
DEFINE_BTECH_COMMAND_INVOKER(set_xcodestuff)
DEFINE_BTECH_COMMAND_INVOKER(show_mechs_damage)
DEFINE_BTECH_COMMAND_INVOKER(tech_checkstatus)
DEFINE_BTECH_COMMAND_INVOKER(tech_fix)
DEFINE_BTECH_COMMAND_INVOKER(tech_fixarmor)
DEFINE_BTECH_COMMAND_INVOKER(tech_fixextra)
DEFINE_BTECH_COMMAND_INVOKER(tech_fixinternal)
DEFINE_BTECH_COMMAND_INVOKER(tech_magic)
DEFINE_BTECH_COMMAND_INVOKER(tech_reattach)
DEFINE_BTECH_COMMAND_INVOKER(tech_reload)
DEFINE_BTECH_COMMAND_INVOKER(tech_removegun)
DEFINE_BTECH_COMMAND_INVOKER(tech_removepart)
DEFINE_BTECH_COMMAND_INVOKER(tech_removesection)
DEFINE_BTECH_COMMAND_INVOKER(tech_repairgun)
DEFINE_BTECH_COMMAND_INVOKER(tech_repairpart)
DEFINE_BTECH_COMMAND_INVOKER(tech_replacegun)
DEFINE_BTECH_COMMAND_INVOKER(tech_replacepart)
DEFINE_BTECH_COMMAND_INVOKER(tech_replacesuit)
DEFINE_BTECH_COMMAND_INVOKER(tech_reseal)
DEFINE_BTECH_COMMAND_INVOKER(tech_toggletype)
DEFINE_BTECH_COMMAND_INVOKER(tech_unload)

#undef DEFINE_BTECH_COMMAND_INVOKER
