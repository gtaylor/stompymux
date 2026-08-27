/* Rebuilds derived unit technology flags from installed equipment. */

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "context_internal.h"
#include "equipment_types.h"
#include "mech_c3_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_network_api.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "template_api.h"
#include "weapon_catalogue_api.h"

static int *template_integer_slot(int *values, size_t count, int index) {
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

static void template_c3_master_state_update(Mech *mech, int c3_master_count) {
  mech_c3_total_masters_set(mech, 0);
  mech_c3_working_masters_set(mech, 0);
  mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED);
  if (c3_master_count <= 0) {
    mech->rd.specials &= ~C3_MASTER_TECH;
    return;
  }
  mech_c3_total_masters_set(mech, mech_c3_total_master_count(mech));
  mech_c3_working_masters_set(mech, mech_c3_working_master_count(mech));
  if (mech_c3_total_masters(mech) > 0)
    ((mech)->rd.specials) |= C3_MASTER_TECH;
  if (mech_c3_working_masters(mech) == 0)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED);
}

typedef struct TemplateSpecialScan {
  int masc_count;
  int c3_master_count;
  int tsm_count;
  int ff_count;
  int es_count;
  int tc_count;
  int stealth_armor_by_section[NUM_SECTIONS];
  int null_signature_by_section[NUM_SECTIONS];
  int stealth_armor_count;
  int null_signature_count;
  int angel_count;
  int heavy_ferro_count;
  int light_ferro_count;
  int c3i_count;
  int bloodhound_count;
  int infantry_special_count[5];
  int suit_count;
  int engine_count;
  bool clan;
} TemplateSpecialScan;

static void template_special_part_scan(Mech *mech, int section, int critical,
                                       TemplateSpecialScan *scan) {
  const int x = section;
  const int y = critical;
  const int t = mech_critical_part_type(mech, x, y);
  if (t) {
    switch (special_from_equipment_index(t)) {
    case ARTEMIS_IV:
      ((mech)->rd.specials) |= ARTEMIS_IV_TECH;
      break;
    case BEAGLE_PROBE:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        ((mech)->rd.specials) |= BEAGLE_PROBE_TECH;
      break;
    case LIGHT_BAP:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        ((mech)->rd.specials) |= LIGHT_BAP_TECH;
      break;
    case ECM:
      ((mech)->rd.specials) |= ECM_TECH;
      break;
    case TAG:
      ((mech)->rd.specials2) |= TAG_TECH;
      break;
    case C3_SLAVE:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        ((mech)->rd.specials) |= C3_SLAVE_TECH;
      break;
    case MASC:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        scan->masc_count++;
      break;
    case C3_MASTER:
      scan->c3_master_count++;
      break;
    case C3I:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        scan->c3i_count++;
      break;
    case ANGELECM:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        scan->angel_count++;
      break;
    case TRIPLE_STRENGTH_MYOMER:
      scan->tsm_count++;
      break;
    case FERRO_FIBROUS:
      scan->ff_count++;
      break;
    case HVY_FERRO_FIBROUS:
      scan->heavy_ferro_count++;
      break;
    case LT_FERRO_FIBROUS:
      scan->light_ferro_count++;
      break;
    case BLOODHOUND_PROBE:
      scan->bloodhound_count++;
      break;
    case TARGETING_COMPUTER:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        scan->tc_count++;
      break;
    case ENDO_STEEL:
      scan->es_count++;
      break;
    case PURIFIER_ARMOR:
      (*template_integer_slot(scan->infantry_special_count, 5, 0))++;
      break;
    case KAGE_STEALTH_UNIT:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        (*template_integer_slot(scan->infantry_special_count, 5, 1))++;
      break;
    case ACHILEUS_STEALTH_UNIT:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        (*template_integer_slot(scan->infantry_special_count, 5, 2))++;
      break;
    case INFILTRATOR_STEALTH_UNIT:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        (*template_integer_slot(scan->infantry_special_count, 5, 3))++;
      break;
    case INFILTRATORII_STEALTH_UNIT:
      if (!mech_critical_is_nonfunctional(mech, x, y))
        (*template_integer_slot(scan->infantry_special_count, 5, 4))++;
      break;
    case ENGINE:
      scan->engine_count++;
      break;
    case CASE:
      mech_section_configuration_add(
          mech, ((mech)->ud.type) == CLASS_VEH_GROUND ? BSIDE : x, CASE_TECH);
      break;
    case STEALTH_ARMOR:
      (*template_integer_slot(scan->stealth_armor_by_section, NUM_SECTIONS,
                              x))++;
      scan->stealth_armor_count++;
      break;
    case NULL_SIGNATURE_SYSTEM:
      (*template_integer_slot(scan->null_signature_by_section, NUM_SECTIONS,
                              x))++;
      scan->null_signature_count++;
      break;
    }
    if (equipment_is_weapon(t) &&
        weapon_catalogue_is_anti_missile(weapon_from_equipment_index(t))) {
      if (weapon_catalogue_has_special(weapon_from_equipment_index(t), CLAT))
        ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
      else
        ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
    }
  }
}

void update_specials(Mech *mech) {
  int x;
  int y;
  int t;
  TemplateSpecialScan scan = {
      .clan = (mech->rd.specials & CLAN_TECH) != 0,
  };
  int t_tech_ok = 1;

  ((mech)->rd.specials) &=
      ~(BEAGLE_PROBE_TECH | TRIPLE_MYOMER_TECH | MASC_TECH | ECM_TECH |
        C3_SLAVE_TECH | C3_MASTER_TECH | ARTEMIS_IV_TECH | ES_TECH | FF_TECH |
        IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH | LIGHT_BAP_TECH);
  if (((mech)->ud.type) == CLASS_MECH)
    ((mech)->rd.specials) &= ~(XL_TECH | XXL_TECH | CE_TECH | LE_TECH);

  ((mech)->rd.specials2) &=
      ~(STEALTH_ARMOR_TECH | NULLSIGSYS_TECH | ANGEL_ECM_TECH |
        HVY_FF_ARMOR_TECH | LT_FF_ARMOR_TECH | TAG_TECH | C3I_TECH |
        BLOODHOUND_PROBE_TECH | TCOMP_TECH);

  ((mech)->rd.infantry_specials) &=
      ~(CS_PURIFIER_STEALTH_TECH | DC_KAGE_STEALTH_TECH |
        FWL_ACHILEUS_STEALTH_TECH | FC_INFILTRATOR_STEALTH_TECH |
        FC_INFILTRATORII_STEALTH_TECH);

  for (x = 0; x < 5; x++)
    *template_integer_slot(scan.infantry_special_count, 5, x) = 0;

  for (x = 0; x < NUM_SECTIONS; x++) {
    scan.engine_count = 0;
    mech_section_configuration_remove(mech, x, CASE_TECH);
    *template_integer_slot(scan.stealth_armor_by_section, NUM_SECTIONS, x) = 0;
    *template_integer_slot(scan.null_signature_by_section, NUM_SECTIONS, x) = 0;

    for (y = 0; y < crits_in_loc(mech, x); y++) {
      template_special_part_scan(mech, x, y, &scan);
    }
    if (x != CTORSO && scan.engine_count) {
      if (scan.engine_count > 3) {
        ((mech)->rd.specials) |= XXL_TECH;

      } else if (scan.engine_count == 2) {
        if (scan.clan)
          ((mech)->rd.specials) |= XL_TECH;

        else
          ((mech)->rd.specials) |= LE_TECH;

      } else {
        ((mech)->rd.specials) |= XL_TECH;
      }
    } else {
      if (x == CTORSO && scan.engine_count < 4 &&
          ((mech)->ud.type) == CLASS_MECH)
        ((mech)->rd.specials) |= CE_TECH;
    }
  }
  if ((((mech)->rd.specials) & (XXL_TECH | XL_TECH | LE_TECH)) &&
      (((mech)->rd.specials) & CE_TECH))
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
        "#%ld apparently is very weird: Compact engine AND XL/XXL?",
        mech->mynum);
  if (scan.tc_count) {
    ((mech)->rd.specials2) |= TCOMP_TECH;
    for (x = 0; x < NUM_SECTIONS; x++) {
      for (y = 0; y < crits_in_loc(mech, x); y++) {
        t = mech_critical_part_type(mech, x, y);
        if (equipment_is_weapon(t))
          if (equipment_can_use_targeting_computer(t))
            mech_critical_fire_mode_add(mech, x, y, ON_TC);
      }
    }
  }
  if (scan.masc_count >= max(1, (((mech)->ud.tons) / (scan.clan ? 25 : 20))))
    ((mech)->rd.specials) |= MASC_TECH;
  if (scan.ff_count >= (scan.clan ? 7 : 14) ||
      (((mech)->ud.type) != CLASS_MECH && scan.ff_count > 0))
    ((mech)->rd.specials) |= FF_TECH;
  if (scan.es_count >= (scan.clan ? 7 : 14) ||
      (((mech)->ud.type) != CLASS_MECH && scan.es_count > 0))
    ((mech)->rd.specials) |= ES_TECH;
  if (scan.tsm_count >= 6 ||
      (((mech)->ud.type) != CLASS_MECH && scan.tsm_count > 0))
    ((mech)->rd.specials) |= TRIPLE_MYOMER_TECH;
  if (scan.angel_count >= 2 ||
      (((mech)->ud.type) != CLASS_MECH && scan.angel_count > 0))
    ((mech)->rd.specials2) |= ANGEL_ECM_TECH;
  if (scan.heavy_ferro_count >= 21 ||
      (((mech)->ud.type) != CLASS_MECH && scan.heavy_ferro_count > 0))
    ((mech)->rd.specials2) |= HVY_FF_ARMOR_TECH;
  if (scan.light_ferro_count >= 7 ||
      (((mech)->ud.type) != CLASS_MECH && scan.light_ferro_count > 0))
    ((mech)->rd.specials2) |= LT_FF_ARMOR_TECH;
  if (scan.c3i_count >= 2 ||
      (((mech)->ud.type) != CLASS_MECH && scan.c3i_count > 0))
    ((mech)->rd.specials2) |= C3I_TECH;
  if (scan.bloodhound_count >= 3 ||
      (((mech)->ud.type) != CLASS_MECH && scan.bloodhound_count > 0))
    ((mech)->rd.specials2) |= BLOODHOUND_PROBE_TECH;

  if (((mech)->ud.type) == CLASS_MECH) {
    /* Be 'noisy' about some crits/techs */
    if ((scan.ff_count > 0) && (scan.ff_count < (scan.clan ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
                         "%s (#%ld) is missing FF Crits %d/%d!",
                         ((mech)->ud.mech_type), mech->mynum, scan.ff_count,
                         (scan.clan ? 7 : 14));

    if ((scan.es_count > 0) && (scan.es_count < (scan.clan ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
                         "%s (#%ld) is missing ES Crits %d/%d!",
                         ((mech)->ud.mech_type), mech->mynum, scan.es_count,
                         (scan.clan ? 7 : 14));

    if ((scan.tsm_count > 0) && (scan.tsm_count < 6))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
                         "%s (#%ld) is missing TSM Crits %d/6!",
                         ((mech)->ud.mech_type), mech->mynum, scan.tsm_count);

    if ((scan.heavy_ferro_count > 0) && (scan.heavy_ferro_count < 21))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
                         "%s (#%ld) is missing HvyFF Crits %d/21!",
                         ((mech)->ud.mech_type), mech->mynum,
                         scan.heavy_ferro_count);

    if ((scan.light_ferro_count > 0) && (scan.light_ferro_count < 7))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS,
                         "%s (#%ld) is missing LtFF Crits %d/7!",
                         ((mech)->ud.mech_type), mech->mynum,
                         scan.light_ferro_count);
  }

  /*
   * Check our NSS. Need 1 crit in each loc except H
   */
  if (scan.null_signature_count > 0) {
    t_tech_ok = 1;

    if (((mech)->ud.type) != CLASS_MECH) {
      if (scan.null_signature_count < 1)
        t_tech_ok = 0;
    } else {
      for (x = 0; x < NUM_SECTIONS; x++) {
        if (x != HEAD) {
          if (*template_integer_slot(scan.null_signature_by_section,
                                     NUM_SECTIONS, x) < 1) {
            t_tech_ok = 0;
            break;
          }
        }
      }
    }

    if (t_tech_ok)
      ((mech)->rd.specials2) |= NULLSIGSYS_TECH;
  }

  /*
   * Check our Stealth armor. Need 2 crits in each loc except H and CT
   */
  if (scan.stealth_armor_count > 0) {
    t_tech_ok = 1;

    if (!(((mech)->rd.specials) & ECM_TECH)) {
      t_tech_ok = 0;
    } else {
      if (((mech)->ud.type) != CLASS_MECH) {
        if (scan.stealth_armor_count < 1)
          t_tech_ok = 0;
      } else {
        for (x = 0; x < NUM_SECTIONS; x++) {
          if ((x != HEAD) && (x != CTORSO)) {
            if (*template_integer_slot(scan.stealth_armor_by_section,
                                       NUM_SECTIONS, x) < 2) {
              t_tech_ok = 0;
              break;
            }
          }
        }
      }
    }

    if (t_tech_ok)
      ((mech)->rd.specials2) |= STEALTH_ARMOR_TECH;
  }

  /* Let's do our suit checks */
  if (((mech)->ud.type) == CLASS_BSUIT) {
    scan.suit_count = bsuit_member_count(mech);

    if (*template_integer_slot(scan.infantry_special_count, 5, 0) >=
        scan.suit_count)
      ((mech)->rd.infantry_specials) |= CS_PURIFIER_STEALTH_TECH;

    if (*template_integer_slot(scan.infantry_special_count, 5, 1) >=
        scan.suit_count)
      ((mech)->rd.infantry_specials) |= DC_KAGE_STEALTH_TECH;

    if (*template_integer_slot(scan.infantry_special_count, 5, 2) >=
        scan.suit_count)
      ((mech)->rd.infantry_specials) |= FWL_ACHILEUS_STEALTH_TECH;

    if (*template_integer_slot(scan.infantry_special_count, 5, 3) >=
        scan.suit_count)
      ((mech)->rd.infantry_specials) |= FC_INFILTRATOR_STEALTH_TECH;

    if (*template_integer_slot(scan.infantry_special_count, 5, 4) >=
        scan.suit_count)
      ((mech)->rd.infantry_specials) |= FC_INFILTRATORII_STEALTH_TECH;
  }

  /* New C3 Master code */
  template_c3_master_state_update(mech, scan.c3_master_count);
}
