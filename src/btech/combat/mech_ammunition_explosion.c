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

void mech_ammunition_explode(const AmmunitionExplosionRequest *request) {
  Mech *attacker = request->attacker;
  Mech *mech = request->target;
  const int AMMUNITION_SECTION = request->ammunition.section;
  const int AMMUNITION_CRITICAL = request->ammunition.critical;
  int damage = request->damage;
  BtechContext *context = mech_context(mech);
  int ammunition_mode =
      mech_critical_ammo_mode(mech, AMMUNITION_SECTION, AMMUNITION_CRITICAL);

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
  mech_critical_destroy(mech, AMMUNITION_SECTION, AMMUNITION_CRITICAL);
  if (!attacker)
    return;
  if (ammunition_mode & INFERNO_MODE) {
    mech_inferno_hit(mech, mech, damage / 4, 0);
    if (btech_context_inferno_penalty_enabled(context))
      mech_weapon_heat_add(mech, 30.0);
    damage = damage / 2;
  }
  if (mech_class(mech) == CLASS_BSUIT) {
    mech_damage_apply(&(MechDamageRequest){.target = mech,
                                           .attacker = attacker,
                                           .line_of_sight = 0,
                                           .attack_pilot = -1,
                                           .hit_location = AMMUNITION_SECTION,
                                           .rear = 0,
                                           .critical = 0,
                                           .armor_damage = damage,
                                           .internal_damage = 0,
                                           .transfer = MECH_DAMAGE_NORMAL,
                                           .cause = -1,
                                           .base_to_hit = 0,
                                           .weapon_index = -1,
                                           .ammunition_mode = 0,
                                           .ignore_swarmers = 0});
  } else {
    mech_damage_apply(
        &(MechDamageRequest){.target = mech,
                             .attacker = attacker,
                             .line_of_sight = 0,
                             .attack_pilot = -1,
                             .hit_location = AMMUNITION_SECTION,
                             .rear = 0,
                             .critical = 0,
                             .armor_damage = 0,
                             .internal_damage = damage,
                             .transfer = MECH_DAMAGE_FORCE_TRANSFER,
                             .cause = -1,
                             .base_to_hit = 0,
                             .weapon_index = -1,
                             .ammunition_mode = 0,
                             .ignore_swarmers = 0});
  }

  if (mech_class(mech) != CLASS_BSUIT) {
    mech_notify(mech, MECHPILOT,
                "You take personal injury from the ammunition explosion!");
    if (has_bool_advantage(context, mech_pilot_dbref(mech), "pain_resistance"))
      headhitmwdamage(mech, mech, 1);
    else
      headhitmwdamage(mech, mech, 2);
  }
}
