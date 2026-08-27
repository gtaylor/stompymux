/* Implements BattleTech combat mechanics for unit hitloc standard. */

#include "aero_move_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "section_types.h"

typedef struct HitLocationOutput {
  bool *critical;
  bool *rear;
} HitLocationOutput;

typedef struct StandardHitRoll {
  int group;
  int roll;
} StandardHitRoll;

static int hit_location_export(HitLocationResult result,
                               HitLocationOutput output) {
  *output.critical = result.critical;
  *output.rear = result.rear;
  return result.location;
}

static int standard_mech_hit_location(Mech *mech, StandardHitRoll hit,
                                      bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  BtechContext *context = mech_context(mech);
  if (mech_class(mech) == CLASS_BSUIT) {
    hitloc = mech_battle_suit_hit_location(mech);
    if (hitloc < 0)
      return btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1);
  }
  switch (hit_group) {
  case LEFTSIDE:
    switch (roll) {
    case 2:
      if (mech_section_is_crittable(mech, LTORSO, (CriticalThreshold){60})) {
        btech_channel_send(
            context, BTECH_CHANNEL_TAC_INFO,
            "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
            mech_dbref(mech));
        *iscritical = true;
      }
      return LTORSO;
    case 3:
      return LLEG;
    case 4:
    case 5:
      return LARM;
    case 6:
      return LLEG;
    case 7:
      return LTORSO;
    case 8:
      return CTORSO;
    case 9:
      return RTORSO;
    case 10:
      return RARM;
    case 11:
      return RLEG;
    case 12:
      if (btech_context_uses_exile_stun_code(context))
        return mech_head_hit_modify(hit_group, mech);
      return HEAD;
    }
    break;
  case RIGHTSIDE:
    switch (roll) {
    case 2:
      if (mech_section_is_crittable(mech, RTORSO, (CriticalThreshold){60})) {
        btech_channel_send(
            context, BTECH_CHANNEL_TAC_INFO,
            "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
            mech_dbref(mech));
        *iscritical = true;
      }
      return RTORSO;
    case 3:
      return RLEG;
    case 4:
    case 5:
      return RARM;
    case 6:
      return RLEG;
    case 7:
      return RTORSO;
    case 8:
      return CTORSO;
    case 9:
      return LTORSO;
    case 10:
      return LARM;
    case 11:
      return LLEG;
    case 12:
      if (btech_context_uses_exile_stun_code(context))
        return mech_head_hit_modify(hit_group, mech);
      return HEAD;
    }
    break;
  case FRONT:
  case BACK:
    switch (roll) {
    case 2:
      if (mech_section_is_crittable(mech, CTORSO, (CriticalThreshold){60})) {
        btech_channel_send(
            context, BTECH_CHANNEL_TAC_INFO,
            "%ld's luck sucks. It got TACed. We're in FindHitLocation()",
            mech_dbref(mech));
        *iscritical = true;
      }
      return CTORSO;
    case 3:
    case 4:
      return RARM;
    case 5:
      return RLEG;
    case 6:
      return RTORSO;
    case 7:
      return CTORSO;
    case 8:
      return LTORSO;
    case 9:
      return LLEG;
    case 10:
    case 11:
      return LARM;
    case 12:
      if (btech_context_uses_exile_stun_code(context))
        return mech_head_hit_modify(hit_group, mech);
      return HEAD;
    }
  }
  return hitloc;
}

static int standard_ground_vehicle_hit_location(Mech *mech, StandardHitRoll hit,
                                                bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  int side;
  switch (hit_group) {
  case LEFTSIDE:
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, LSIDE, (CriticalThreshold){40}))
        *iscritical = true;
      return LSIDE;
    case 3:
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      return LSIDE;
    case 10:
      return (mech_section_internal(mech, TURRET)) ? TURRET : LSIDE;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        if (mech_section_is_crittable(mech, TURRET, (CriticalThreshold){50}))
          *iscritical = true;
        return TURRET;
      } else {
        return LSIDE;
      }
    }
    break;
  case RIGHTSIDE:
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, RSIDE, (CriticalThreshold){40}))
        *iscritical = true;
      return RSIDE;
    case 3:
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      return RSIDE;
    case 10:
      return (mech_section_internal(mech, TURRET)) ? TURRET : RSIDE;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        if (mech_section_is_crittable(mech, TURRET, (CriticalThreshold){50}))
          *iscritical = true;
        return TURRET;
      } else {
        return RSIDE;
      }
    }
    break;

  case FRONT:
  case BACK:
    side = (hit_group == FRONT ? FSIDE : BSIDE);
    switch (roll) {
    case 2:
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){40}))
        *iscritical = true;
      return side;
    case 12:
      if (mech_section_internal(mech, TURRET)) {
        if (mech_section_is_crittable(mech, TURRET, (CriticalThreshold){50}))
          *iscritical = true;
        return TURRET;
      }
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){40}))
        *iscritical = true;
      return side;
    case 3:
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      return side;
    case 10:
      return (mech_section_internal(mech, TURRET)) ? TURRET : side;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        if (mech_section_is_crittable(mech, TURRET, (CriticalThreshold){50}))
          *iscritical = true;
        return TURRET;
      } else {
        return side;
      }
    }
  }
  return hitloc;
}

static int standard_aero_hit_location(Mech *mech, StandardHitRoll hit,
                                      bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  int side;
  switch (hit_group) {
  case FRONT:
    switch (roll) {
    case 2:
    case 12:
    case 3:
    case 11:
      if (mech_section_is_crittable(mech, AERO_NOSE, (CriticalThreshold){90}))
        mech_weapon_destroy_random(mech, AERO_NOSE);
      return AERO_NOSE;
    case 4:
    case 10:
    case 5:
      return AERO_RWING;
    case 9:
      return AERO_LWING;
    case 6:
    case 7:
    case 8:
      return AERO_NOSE;
    }
    break;
  case LEFTSIDE:
  case RIGHTSIDE:
    side = ((hit_group == LEFTSIDE) ? AERO_LWING : AERO_RWING);
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, AERO_AFT, (CriticalThreshold){99}))
        *iscritical = true;
      return AERO_AFT;
    case 3:
    case 11:
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){99}))
        mech_weapon_destroy_random(mech, side);
      return side;
    case 4:
    case 10:
      if (mech_section_is_crittable(mech, AERO_AFT, (CriticalThreshold){90}))
        mech_heat_sink_destroy(mech, AERO_AFT);
      return AERO_AFT;
    case 5:
    case 9:
      return AERO_NOSE;
    case 6:
    case 8:
      return side;
    case 7:
      return side;
    }
    break;
  case BACK:
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, AERO_AFT, (CriticalThreshold){90}))
        *iscritical = true;
      return AERO_AFT;
    case 3:
    case 11:
    case 4:
    case 7:
    case 10:
    case 5:
      return AERO_RWING;
    case 9:
      return AERO_LWING;
    case 6:
    case 8:
      return AERO_AFT;
    }
  }
  return hitloc;
}

static int standard_dropship_hit_location(Mech *mech, StandardHitRoll hit,
                                          bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);
  switch (hit_group) {
  case FRONT:
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){30}))
        dropship_bridge_hit(mech);
      return DS_NOSE;
    case 3:
    case 11:
      if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){50}))
        mech_weapon_destroy_random(mech, DS_NOSE);
      return DS_NOSE;
    case 5:
      return DS_RWING;
    case 6:
    case 7:
    case 8:
      return DS_NOSE;
    case 9:
      return DS_LWING;
    case 4:
    case 10:
      return (btech_random_range(context, 1, 2)) == 1 ? DS_LWING : DS_RWING;
    }
    break;
  case LEFTSIDE:
  case RIGHTSIDE:
    side = (hit_group == LEFTSIDE) ? DS_LWING : DS_RWING;
    if (btech_random_range(context, 1, 2) == 2)
      side = mech_spheroid_rear_section(mech, side);
    switch (roll) {
    case 2:
      if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){30}))
        dropship_bridge_hit(mech);
      return DS_NOSE;
    case 3:
    case 11:
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){60}))
        mech_weapon_destroy_random(mech, side);
      return side;
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
    case 10:
      return side;
    case 9:
      return DS_NOSE;
    case 12:
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){60}))
        *iscritical = true;
      return side;
    }
    break;
  case BACK:
    switch (roll) {
    case 2:
    case 12:
      if (mech_section_is_crittable(mech, DS_AFT, (CriticalThreshold){60}))
        *iscritical = true;
      return DS_AFT;
    case 3:
    case 11:
      return DS_AFT;
    case 4:
    case 7:
    case 10:
      if (mech_section_is_crittable(mech, DS_AFT, (CriticalThreshold){60}))
        mech_heat_sink_destroy(mech, DS_AFT);
      return DS_AFT;
    case 5:
      hitloc = DS_RWING;
      hitloc = mech_spheroid_rear_section(mech, hitloc);
      return hitloc;
    case 6:
    case 8:
      return DS_AFT;
    case 9:
      hitloc = DS_LWING;
      hitloc = mech_spheroid_rear_section(mech, hitloc);
      return hitloc;
    }
  }
  return hitloc;
}

static int standard_vtol_hit_location(Mech *mech, StandardHitRoll hit,
                                      bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  int side;
  switch (hit_group) {
  case LEFTSIDE:
    switch (roll) {
    case 2:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_destroyed_critical_apply(mech, nullptr, 1);
      break;
    case 3:
    case 4:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      hitloc = LSIDE;
      break;
    case 10:
    case 11:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 12:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    }
    break;

  case RIGHTSIDE:
    switch (roll) {
    case 2:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_destroyed_critical_apply(mech, nullptr, 1);
      break;
    case 3:
    case 4:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      hitloc = RSIDE;
      break;
    case 10:
    case 11:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 12:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    }
    break;

  case FRONT:
  case BACK:
    side = (hit_group == FRONT ? FSIDE : BSIDE);
    switch (roll) {
    case 2:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_destroyed_critical_apply(mech, nullptr, 1);
      break;
    case 3:
    case 4:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      hitloc = side;
      break;
    case 10:
    case 11:
      hitloc = ROTOR;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    case 12:
      hitloc = ROTOR;
      *iscritical = true;
      mech_vtol_rotor_damaged_critical_apply(mech);
      break;
    }
    break;
  }
  return hitloc;
}

static int standard_naval_hit_location(Mech *mech, StandardHitRoll hit,
                                       bool *iscritical) {
  int hit_group = hit.group;
  int roll = hit.roll;
  int hitloc = 0;
  int side;
  switch (hit_group) {
  case LEFTSIDE:
    switch (roll) {
    case 2:
      hitloc = LSIDE;
      if (mech_section_is_crittable(mech, hitloc, (CriticalThreshold){40}))
        *iscritical = true;
      break;
    case 3:
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
      hitloc = LSIDE;
      break;
    case 9:
      hitloc = LSIDE;
      break;
    case 10:
      if (mech_section_internal(mech, TURRET))
        hitloc = TURRET;
      else
        hitloc = LSIDE;
      break;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        hitloc = TURRET;
        if (mech_section_is_crittable(mech, hitloc, (CriticalThreshold){40}))
          *iscritical = true;
      } else {
        hitloc = LSIDE;
      }
      break;
    case 12:
      hitloc = LSIDE;
      if (mech_section_is_crittable(mech, hitloc, (CriticalThreshold){40}))
        *iscritical = true;
      break;
    }
    break;

  case RIGHTSIDE:
    switch (roll) {
    case 2:
    case 12:
      hitloc = RSIDE;
      if (mech_section_is_crittable(mech, hitloc, (CriticalThreshold){40}))
        *iscritical = true;
      break;
    case 3:
    case 4:
    case 5:
    case 6:
    case 7:
    case 8:
    case 9:
      hitloc = RSIDE;
      break;
    case 10:
      if (mech_section_internal(mech, TURRET))
        hitloc = TURRET;
      else
        hitloc = RSIDE;
      break;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        hitloc = TURRET;
        if (mech_section_is_crittable(mech, hitloc, (CriticalThreshold){40}))
          *iscritical = true;
      } else {
        hitloc = RSIDE;
      }
      break;
    }
    break;

  case FRONT:
  case BACK:
    side = (hit_group == FRONT ? FSIDE : BSIDE);
    switch (roll) {
    case 2:
      hitloc = side;
      if (mech_section_is_crittable(mech, side, (CriticalThreshold){40}))
        *iscritical = true;
      break;
    case 12:
      if (mech_section_internal(mech, TURRET)) {
        hitloc = TURRET;
        if (mech_section_is_crittable(mech, TURRET, (CriticalThreshold){50}))
          *iscritical = true;
      } else {
        hitloc = side;
        if (mech_section_is_crittable(mech, side, (CriticalThreshold){40}))
          *iscritical = true;
      }
      break;
    case 3:
      hitloc = side;
      break;
    case 4:
      hitloc = side;
      break;
    case 5:
      hitloc = side;
      break;
    case 6:
    case 7:
    case 8:
    case 9:
      hitloc = side;
      break;
    case 10:
      if (mech_section_internal(mech, TURRET))
        hitloc = TURRET;
      else
        hitloc = side;
      break;
    case 11:
      if (mech_section_internal(mech, TURRET)) {
        hitloc = TURRET;
        *iscritical = true;
      } else {
        hitloc = side;
      }
      break;
    }
    break;
  }
  return hitloc;
}
int mech_hit_location(Mech *mech, int hit_group, bool *iscritical,
                      bool *isrear) {
  int roll;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  roll = btech_random_roll(context);

  /* We have a varying set of crit charts we can use, so let's see what's been
   * config'd */
  /* The advanced FASA table already checks critical immunity; the others do
   * not, so dispatch to the crit-proof table after that check. */
  switch (mech_class(mech)) {
  case CLASS_VTOL:
    if (btech_context_uses_advanced_vtol_criticals(context)) {
      return hit_location_export(
          mech_advanced_vehicle_hit_location(
              mech, hit_group,
              (HitLocationResult){.critical = (*iscritical) != 0,
                                  .rear = (*isrear) != 0}),
          (HitLocationOutput){iscritical, isrear});
    } else if (mech_technology_flags(mech) & CRITPROOF_TECH) {
      return mech_critproof_hit_location(mech, hit_group, iscritical);
    } else if (btech_context_uses_fasa_criticals(context)) {
      return hit_location_export(
          mech_fasa_hit_location(
              mech, hit_group,
              (HitLocationResult){.critical = (*iscritical) != 0,
                                  .rear = (*isrear) != 0}),
          (HitLocationOutput){iscritical, isrear});
    }
    break;
  case CLASS_VEH_GROUND:
    if (btech_context_uses_advanced_vehicle_criticals(context)) {
      return hit_location_export(
          mech_advanced_vehicle_hit_location(
              mech, hit_group,
              (HitLocationResult){.critical = (*iscritical) != 0,
                                  .rear = (*isrear) != 0}),
          (HitLocationOutput){iscritical, isrear});
    } else if (mech_technology_flags(mech) & CRITPROOF_TECH) {
      return mech_critproof_hit_location(mech, hit_group, iscritical);
    } else if (btech_context_uses_fasa_criticals(context)) {
      return hit_location_export(
          mech_fasa_hit_location(
              mech, hit_group,
              (HitLocationResult){.critical = (*iscritical) != 0,
                                  .rear = (*isrear) != 0}),
          (HitLocationOutput){iscritical, isrear});
    }
    break;
  case CLASS_MECH:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_AERO:
  case CLASS_MW:
  case CLASS_DS:
  case CLASS_BSUIT:
  default:
    if (mech_technology_flags(mech) & CRITPROOF_TECH) {
      return mech_critproof_hit_location(mech, hit_group, iscritical);
    } else if (btech_context_uses_fasa_criticals(context)) {
      return hit_location_export(
          mech_fasa_hit_location(
              mech, hit_group,
              (HitLocationResult){.critical = (*iscritical) != 0,
                                  .rear = (*isrear) != 0}),
          (HitLocationOutput){iscritical, isrear});
    }
    break;
  }

  if (condition.combat_safe)
    return 0;
  if (condition.dug_in && mech_section_original_internal(mech, TURRET) &&
      btech_random_range(context, 1, 100) >= 42)
    return TURRET;
  btech_context_hit_roll_record(context, roll);
  switch (mech_class(mech)) {
  case CLASS_BSUIT:
  case CLASS_MW:
  case CLASS_MECH:
    return standard_mech_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  case CLASS_VEH_GROUND:
    return standard_ground_vehicle_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  case CLASS_AERO:
    return standard_aero_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return standard_dropship_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  case CLASS_VTOL:
    return standard_vtol_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  case CLASS_VEH_NAVAL:
    return standard_naval_hit_location(
        mech, (StandardHitRoll){.group = hit_group, .roll = roll}, iscritical);
  }
  return 0;
}
