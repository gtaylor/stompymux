#include "mech_update_internal.h"

#include "mech_identity_api.h"

void CheckDamage(Mech *wounded) {
  int now = wounded->xcode.context->events->tick % TURN;

  if (!btech_context_stagger_mode(mech_context(wounded))) {
    if (!IsDS(wounded) && MechTurnDamage(wounded) >= 20 &&
        (!MechStaggeredLastTurn(wounded) || MechStaggerStamp(wounded) == now)) {
      if (!Jumping(wounded) && !Fallen(wounded) && !OODing(wounded)) {
        mech_notify(wounded, MECHALL, "You stagger from the damage!");
        if (!MadePilotSkillRoll(wounded, 1)) {
          mech_notify(wounded, MECHALL, "You fall over from all the damage!");
          mech_los_broadcast(wounded, "falls down, staggered by the damage!");
          MechFalls(wounded, 1, 0);
        }
      }
      MechTurnDamage(wounded) = 0;
      SetMechStaggerStamp(wounded, now);
      return;
    }
    if ((MechStaggeredLastTurn(wounded) && MechStaggerStamp(wounded) == now) ||
        (!MechStaggeredLastTurn(wounded) && !now)) {
      MechTurnDamage(wounded) = 0;
      SetMechStaggerStamp(wounded, -1);
    }
  }
}
