/* Implements BattleTech combat mechanics for unit hitloc fasa mech. */

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "section_types.h"

int fasa_mech_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear, int roll) {
  int hitloc = 0;
  BtechContext *context = mech_context(mech);

  switch (mech_class(mech)) {
  case CLASS_BSUIT:
    if ((hitloc = mech_battle_suit_hit_location(mech)) < 0)
      return btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1);
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
        *iscritical = 1;
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
          return mech_head_hit_modify(hitGroup, mech);
        return HEAD;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        btech_channel_send(context, BTECH_CHANNEL_TAC_INFO, "%s",
                           tprintf("%ld's luck sucks. It got TACed. "
                                   "We're in mech_fasa_hit_location()",
                                   mech_dbref(mech)));
        *iscritical = 1;
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
          return mech_head_hit_modify(hitGroup, mech);
        return HEAD;
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
        *iscritical = 1;
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
          return mech_head_hit_modify(hitGroup, mech);
        return HEAD;
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
  return hitloc;
}
