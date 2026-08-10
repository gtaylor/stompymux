/* Implements BattleTech combat mechanics for unit hitloc fasa mech. */

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "section_types.h"

HitLocationResult fasa_mech_hit_location(Mech *mech, int hitGroup,
                                         HitLocationResult result, int roll) {
  int hitloc = 0;
  BtechContext *context = mech_context(mech);

  switch (mech_class(mech)) {
  case CLASS_BSUIT:
    hitloc = mech_battle_suit_hit_location(mech);
    if (hitloc < 0)
      return hit_location_result_at(
          result, btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1));
    [[fallthrough]];
  case CLASS_MW:
  case CLASS_MECH:
    switch (hitGroup) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in mech_fasa_hit_location()",
                                   mech_dbref(mech)));
        result.critical = 1;
        return hit_location_result_at(result, LTORSO);
      case 3:
        return hit_location_result_at(result, LLEG);
      case 4:
      case 5:
        return hit_location_result_at(result, LARM);
      case 6:
        return hit_location_result_at(result, LLEG);
      case 7:
        return hit_location_result_at(result, LTORSO);
      case 8:
        return hit_location_result_at(result, CTORSO);
      case 9:
        return hit_location_result_at(result, RTORSO);
      case 10:
        return hit_location_result_at(result, RARM);
      case 11:
        return hit_location_result_at(result, RLEG);
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return hit_location_result_at(result,
                                        mech_head_hit_modify(hitGroup, mech));
        return hit_location_result_at(result, HEAD);
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in mech_fasa_hit_location()",
                                   mech_dbref(mech)));
        result.critical = 1;
        return hit_location_result_at(result, RTORSO);
      case 3:
        return hit_location_result_at(result, RLEG);
      case 4:
      case 5:
        return hit_location_result_at(result, RARM);
      case 6:
        return hit_location_result_at(result, RLEG);
      case 7:
        return hit_location_result_at(result, RTORSO);
      case 8:
        return hit_location_result_at(result, CTORSO);
      case 9:
        return hit_location_result_at(result, LTORSO);
      case 10:
        return hit_location_result_at(result, LARM);
      case 11:
        return hit_location_result_at(result, LLEG);
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return hit_location_result_at(result,
                                        mech_head_hit_modify(hitGroup, mech));
        return hit_location_result_at(result, HEAD);
      }
      break;
    case FRONT:
    case BACK:
      switch (roll) {
      case 2:
        btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in mech_fasa_hit_location()",
                                   mech_dbref(mech)));
        result.critical = 1;
        return hit_location_result_at(result, CTORSO);
      case 3:
      case 4:
        return hit_location_result_at(result, RARM);
      case 5:
        return hit_location_result_at(result, RLEG);
      case 6:
        return hit_location_result_at(result, RTORSO);
      case 7:
        return hit_location_result_at(result, CTORSO);
      case 8:
        return hit_location_result_at(result, LTORSO);
      case 9:
        return hit_location_result_at(result, LLEG);
      case 10:
      case 11:
        return hit_location_result_at(result, LARM);
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return hit_location_result_at(result,
                                        mech_head_hit_modify(hitGroup, mech));
        return hit_location_result_at(result, HEAD);
      }
    }
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_AERO:
  case CLASS_DS:
    break;
  }
  return hit_location_result_at(result, hitloc);
}
