#include "mech_ammunition_explosion_api.h"

#include "btech/context.h"
#include "btechstats_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "section_types.h"

void mech_ammunition_explode(Mech *attacker, Mech *mech, int ammunition_section,
                             int ammunition_critical, int damage) {
  BtechContext *context = mech_context(mech);
  int ammunition_mode =
      mech_critical_ammo_mode(mech, ammunition_section, ammunition_critical);

  if (mech_class(mech) == CLASS_MW) {
    mech_notify(mech, MECHALL, "Your weapon's ammo explodes!");
    mech_los_broadcast(mech, "'s weapon's ammo explodes!");
  } else {
    mech_notify(mech, MECHALL, "Ammunition explosion!");
    if (ammunition_mode & INFERNO_MODE)
      mech_los_broadcast(mech,
                         "is suddenly enveloped by a brilliant fireball!");
    else
      mech_los_broadcast(mech, "has an internal ammo explosion!");
  }
  mech_critical_destroy(mech, ammunition_section, ammunition_critical);
  if (!attacker)
    return;
  if (ammunition_mode & INFERNO_MODE) {
    mech_inferno_hit(mech, mech, damage / 4, 0);
    if (btech_context_inferno_penalty_enabled(context))
      mech_weapon_heat_add(mech, 30.0);
    damage = damage / 2;
  }
  if (mech_class(mech) == CLASS_BSUIT)
    DamageMech(mech, attacker, 0, -1, ammunition_section, 0, 0, damage, 0, -1,
               0, -1, 0, 0);
  else
    DamageMech(mech, attacker, 0, -1, ammunition_section, 0, 0, -1, damage, -1,
               0, -1, 0, 0);

  if (mech_class(mech) != CLASS_BSUIT) {
    mech_notify(mech, MECHPILOT,
                "You take personal injury from the ammunition explosion!");
    if (HasBoolAdvantage(context, mech_pilot_dbref(mech), "pain_resistance"))
      headhitmwdamage(mech, mech, 1);
    else
      headhitmwdamage(mech, mech, 2);
  }
}
