#include "mech_tech_damages.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_damages_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct RepairDamage {
  int type;
  int location;
  int detail;
} RepairDamage;
typedef struct RepairDamageTable {
  RepairDamage entries[REPAIR_DAMAGE_CAPACITY];
  int count;
} RepairDamageTable;
static RepairDamage *repair_damage(RepairDamageTable *damages, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(damages->entries, REPAIR_DAMAGE_CAPACITY,
                            sizeof(*damages->entries), (size_t)index);
}
static const RepairDamage *repair_damage_const(const RepairDamageTable *damages,
                                               int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(damages->entries, REPAIR_DAMAGE_CAPACITY,
                                  sizeof(*damages->entries), (size_t)index);
}
static void append_damage(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
static void append_damage(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;
  if (len >= size)
    return;
  va_start(ap, fmt);
  char *destination = checked_storage_at(buffer, size, sizeof(*buffer), len);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(destination, size - len, fmt, ap);
  va_end(ap);
}
static const char *const REPAIR_NEED_MSGS[] = {
    "Reattachment",
    "Repairs on %s",
    "Repairs on %s",
    "Repairs on %s",
    "Realign focus on %s",
    "Charging crystal repairs on %s",
    "Barrel repairs on %s",
    "Ammo feed repairs on %s",
    "Ranging system repairs on %s",
    "Ammo feed repairs on %s",
    "Replacement of %s",
    "Reload of %s%s (%d rounds)",
    "Repairs on%s armor (%d points)",
    "Repairs on rear%s armor (%d points)",
    "Repairs on%s internals (%d points)",
    "Removal of section",
    "Removal of %s",
    "Removal of %s",
    "Unload of %s%s(%d rounds)",
    "Reseal",
    "Replace suit",
};
static const char *repair_need_message(int type) {
  if (type < 0)
    abort();
  const char *const *message = (const char *const *)checked_storage_at_const(
      (const void *)REPAIR_NEED_MSGS,
      sizeof(REPAIR_NEED_MSGS) / sizeof(*REPAIR_NEED_MSGS),
      sizeof(*REPAIR_NEED_MSGS), (size_t)type);
  return *message;
}
static void repair_damage_add(RepairDamageTable *damages, int type,
                              int location) {
  *repair_damage(damages, damages->count++) =
      (RepairDamage){.type = type, .location = location};
}
static void repair_damage_add_detail(RepairDamageTable *damages, int type,
                                     int location, int detail) {
  *repair_damage(damages, damages->count++) =
      (RepairDamage){.type = type, .location = location, .detail = detail};
}
static int clan_modified_time(const Mech *mech, int time) {
  return max(1, time / ((mech_technology_flags(mech) & CLAN_TECH) ? 2 : 1));
}
static bool check_for_damage(RepairDamageTable *damages, Mech *mech, int loc) {
  int a;
  int b;
  if (mech_section_is_destroyed(mech, loc)) {
    if (mech_class(mech) != CLASS_BSUIT)
      repair_damage_add(damages, REATTACH, loc);
    else
      repair_damage_add(damages, REPLACESUIT, loc);
    return false;
  }
  /* Added by Kipsta, 8/4/99. */
  if (mech_section_is_flooded(mech, loc)) {
    repair_damage_add(damages, RESEAL, loc);
    return false;
  }
  a = mech_section_internal(mech, loc);
  b = mech_section_original_internal(mech, loc);
  if (a != b) {
    repair_damage_add_detail(damages, FIXINTERNAL, loc, (b - a));
  } else {
    a = mech_section_armor(mech, loc);
    b = mech_section_original_armor(mech, loc);
    if (a != b)
      repair_damage_add_detail(damages, FIXARMOR, loc, (b - a));
    a = mech_section_rear_armor(mech, loc);
    b = mech_section_original_rear_armor(mech, loc);
    if (a != b)
      repair_damage_add_detail(damages, FIXARMOR_R, loc, (b - a));
  }
  for (a = 0; a < NUM_CRITICALS; a++) {
    b = mech_critical_part_type(mech, loc, a);
    if (!b)
      continue;
    if (equipment_is_ammunition(b) &&
        !mech_critical_is_destroyed(mech, loc, a) &&
        mech_critical_data(mech, loc, a) !=
            mech_critical_full_ammunition(mech, loc, a))
      repair_damage_add_detail(damages, RELOAD, loc, a);
    if (!mech_critical_is_nonfunctional(mech, loc, a) &&
        !mech_critical_temporary_failure(mech, loc, a) &&
        !mech_critical_is_damaged(mech, loc, a))
      continue;
    if (mech_part_is_structural_placeholder(b))
      continue;
    /* Destroyed / tempnuke'd part. Either case, it works for us :) */
    if (mech_critical_is_damaged(mech, loc, a)) {
      if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_EN_FOCUS)
        repair_damage_add_detail(damages, ENHCRIT_FOCUS, loc, a);
      else if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_EN_CRYSTAL)
        repair_damage_add_detail(damages, ENHCRIT_CRYSTAL, loc, a);
      else if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_BALL_BARREL)
        repair_damage_add_detail(damages, ENHCRIT_BARREL, loc, a);
      else if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_BALL_AMMO)
        repair_damage_add_detail(damages, ENHCRIT_AMMOB, loc, a);
      else if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_MSL_RANGING)
        repair_damage_add_detail(damages, ENHCRIT_RANGING, loc, a);
      else if (mech_critical_damage_flags(mech, loc, a) & WEAP_DAM_MSL_AMMO)
        repair_damage_add_detail(damages, ENHCRIT_AMMOM, loc, a);
      else
        repair_damage_add_detail(damages, ENHCRIT_MISC, loc, a);
    } else if (equipment_is_weapon(b) &&
               !mech_critical_is_destroyed(mech, loc, a)) {
      repair_damage_add_detail(damages, REPAIRP_T, loc, a);
    } else {
      repair_damage_add_detail(
          damages, equipment_is_weapon(b) ? REPAIRG : REPAIRP, loc, a);
    }
    if (equipment_is_weapon(b))
      a += get_weapon_crits(mech, weapon_from_equipment_index(b)) - 1;
  }
  return true;
}
static bool check_for_scrappage(RepairDamageTable *damages, Mech *mech,
                                int loc) {
  int a;
  int b;
  int ret = 1;
  if (mech_section_is_destroyed(mech, loc))
    return true;
  if (someone_scrapping_loc(mech, loc)) {
    repair_damage_add(damages, DETACH, loc);
    return true;
  }
  for (a = 0; a < NUM_CRITICALS; a++) {
    b = mech_critical_part_type(mech, loc, a);
    if (!b)
      continue;
    if (mech_critical_is_broken(mech, loc, a))
      continue;
    if (mech_part_is_structural_placeholder(b))
      continue;
    if (equipment_is_ammunition(b) && mech_critical_data(mech, loc, a)) {
      repair_damage_add_detail(damages, UNLOAD, loc, a);
      if (ret && !someone_repairing(mech, loc, a))
        ret = 0;
      continue;
    }
    repair_damage_add_detail(damages, equipment_is_weapon(b) ? SCRAPG : SCRAPP,
                             loc, a);
    if (ret && !someone_scrapping_part(mech, loc, a))
      ret = 0;
    if (equipment_is_weapon(b))
      a += get_weapon_crits(mech, weapon_from_equipment_index(b)) - 1;
  }
  if (ret && !invalid_scrap_path(mech, loc))
    repair_damage_add(damages, DETACH, loc);
  return false;
}
static void make_scrap_table(RepairDamageTable *damages, Mech *mech) {
  int i = 4;
  damages->count = 0;
  if (mech_class(mech) == CLASS_MECH) {
    if (check_for_scrappage(damages, mech, RARM))
      i -= check_for_scrappage(damages, mech, RTORSO) ? 1 : 0;
    if (check_for_scrappage(damages, mech, LARM))
      i -= check_for_scrappage(damages, mech, LTORSO) ? 1 : 0;
    i -= check_for_scrappage(damages, mech, RLEG) ? 1 : 0;
    i -= check_for_scrappage(damages, mech, LLEG) ? 1 : 0;
    if (!i)
      check_for_scrappage(damages, mech, CTORSO);
    check_for_scrappage(damages, mech, HEAD);
  } else {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(mech, i))
        check_for_scrappage(damages, mech, i);
  }
}
static void make_damage_table(RepairDamageTable *damages, Mech *mech) {
  int i;
  damages->count = 0;
  if (mech_class(mech) == CLASS_MECH) {
    if (check_for_damage(damages, mech, CTORSO)) {
      if (check_for_damage(damages, mech, LTORSO)) {
        check_for_damage(damages, mech, LARM);
      }
      if (check_for_damage(damages, mech, RTORSO)) {
        check_for_damage(damages, mech, RARM);
      }
      check_for_damage(damages, mech, LLEG);
      check_for_damage(damages, mech, RLEG);
      check_for_damage(damages, mech, HEAD);
    }
  } else {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(mech, i))
        check_for_damage(damages, mech, i);
  }
}
static int is_under_repair(const RepairDamageTable *damages, Mech *mech,
                           int i) {
  const RepairDamage *damage = repair_damage_const(damages, i);
  int v1 = damage->location;
  int v2 = damage->detail;
  switch (damage->type) {
  case RELOAD:
  case REPAIRP:
  case REPAIRP_T:
  case REPAIRG:
  case UNLOAD:
  case ENHCRIT_MISC:
  case ENHCRIT_FOCUS:
  case ENHCRIT_CRYSTAL:
  case ENHCRIT_BARREL:
  case ENHCRIT_AMMOB:
  case ENHCRIT_RANGING:
  case ENHCRIT_AMMOM:
    return someone_repairing(mech, v1, v2);
  case REATTACH:
    return someone_attaching(mech, v1);
  case RESEAL:
    return someone_resealing(mech, v1);
  case FIXARMOR_R:
    return someone_fixing(mech, v1 + 8);
  case FIXARMOR:
  case FIXINTERNAL:
    return someone_fixing(mech, v1);
  case DETACH:
    return someone_scrapping_loc(mech, v1);
  case SCRAPP:
  case SCRAPG:
    return someone_scrapping_part(mech, v1, v2);
  case REPLACESUIT:
    return someone_replacing_suit(mech, v1);
  }
  return 0;
}
void mech_repair_jobs_format(Mech *mech, char *buffer, size_t buffer_size) {
  RepairDamageTable damages_storage = {0};
  RepairDamageTable *damages = &damages_storage;
  int i;
  if (unit_is_fixable(mech))
    make_damage_table(damages, mech);
  else
    make_scrap_table(damages, mech);
  if (buffer_size == 0)
    return;
  buffer[0] = '\0';
  if (!damages->count)
    return;
  for (i = 0; i < damages->count; i++) {
    const RepairDamage *damage = repair_damage_const(damages, i);
    /* Desired format: repairnum|location|typenum|data|fixing? */
    if (i)
      append_damage(buffer, buffer_size, ",");
    append_damage(
        buffer, buffer_size, "%d|%s|%d|", i + 1,
        armor_section_abbreviation(
            &(ArmorSectionReference){.unit_class = mech_class(mech),
                                     .movement_type = mech_movement_type(mech),
                                     .location = damage->location})
            .text,
        damage->type);
    switch (damage->type) {
    case REPAIRP:
    case REPAIRP_T:
    case REPAIRG:
    case ENHCRIT_MISC:
    case ENHCRIT_FOCUS:
    case ENHCRIT_CRYSTAL:
    case ENHCRIT_BARREL:
    case ENHCRIT_AMMOB:
    case ENHCRIT_RANGING:
    case ENHCRIT_AMMOM:
    case SCRAPP:
    case SCRAPG:
      append_damage(buffer, buffer_size, "%s",
                    pos_part_name(mech, damage->location, damage->detail).text);
      break;
    case RELOAD:
      append_damage(
          buffer, buffer_size, "%s:%d",
          pos_part_name(mech, damage->location, damage->detail).text,
          full_ammo(mech, damage->location, damage->detail) -
              mech_critical_data(mech, damage->location, damage->detail));
      break;
    case UNLOAD:
      append_damage(buffer, buffer_size, "%s:%d",
                    pos_part_name(mech, damage->location, damage->detail).text,
                    mech_critical_data(mech, damage->location, damage->detail));
      break;
    case FIXARMOR:
    case FIXARMOR_R:
    case FIXINTERNAL:
      append_damage(buffer, buffer_size, "%d", damage->detail);
      break;
    default:
      append_damage(buffer, buffer_size, "-");
    }
    append_damage(buffer, buffer_size, "|%d",
                  is_under_repair(damages, mech, i));
  }
}
size_t mech_repair_job_count(Mech *mech) {
  RepairDamageTable damages = {0};
  if (unit_is_fixable(mech))
    make_damage_table(&damages, mech);
  else
    make_scrap_table(&damages, mech);
  return (size_t)damages.count;
}
void show_mechs_damage(DbRef player, void *data,
                       char *buffer [[maybe_unused]]) {
  char message_buffer[LBUF_SIZE];
  Mech *mech = data;
  RepairDamageTable damages_storage = {0};
  RepairDamageTable *damages = &damages_storage;
  CoolMenu *c = nullptr;
  int i;
  int j;
  int v1;
  int v2;
  char buf[MBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  char buf3[MBUF_SIZE] = {0};
  int fix_time = 0;
  int fix_bth = 0;
  int extra_hard = 1;
  RepairCommandContext repair_command;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_CONFIGURED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  if (unit_is_fixable(mech))
    make_damage_table(damages, mech);
  else
    make_scrap_table(damages, mech);
  if (!damages->count && mech_class(mech) == CLASS_MECH) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The 'mech is in pristine condition!");
    return;
  }
  if (!damages->count) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "It's in pristine condition!");
    return;
  }
  cool_menu_add_line(&c);
  (void)snprintf(message_buffer, sizeof(message_buffer), "Damage for %s",
                 mech_display_id(mech).text);
  cool_menu_add_centered(&c, message_buffer);
  cool_menu_add_line(&c);
  cool_menu_add_text(&c, "   Fix# Time  BTH Loc Description");
  for (i = 0; i < damages->count; i++) {
    const RepairDamage *damage = repair_damage_const(damages, i);
    v1 = damage->location;
    v2 = damage->detail;
    switch (damage->type) {
    case REATTACH:
      fix_bth = find_tech_skill(player, mech) + REATTACH_DIFFICULTY;
      fix_time = REATTACH_TIME;
      (void)string_copy_bounded(buf, sizeof(buf),
                                repair_need_message(damage->type));
      break;
    case DETACH:
      fix_bth = find_tech_skill(player, mech) + REMOVES_DIFFICULTY;
      fix_time = REMOVES_TIME;
      (void)string_copy_bounded(buf, sizeof(buf),
                                repair_need_message(damage->type));
      break;
    case RESEAL:
      fix_bth = find_tech_skill(player, mech) + RESEAL_DIFFICULTY;
      fix_time = RESEAL_TIME;
      (void)string_copy_bounded(buf, sizeof(buf),
                                repair_need_message(damage->type));
      break;
    case REPLACESUIT:
      (void)string_copy_bounded(buf, sizeof(buf),
                                repair_need_message(damage->type));
      fix_time = REPLACESUIT_TIME;
      fix_bth = find_tech_skill(player, mech) + REPLACESUIT_DIFFICULTY;
      break;
    case REPAIRP:
      fix_bth =
          find_tech_skill(player, mech) + REPLACE_DIFFICULTY +
          repair_part_type_difficulty(mech_critical_part_type(mech, v1, v2));
      fix_time = REPLACEPART_TIME;
      (void)snprintf(buf, sizeof(buf), "Repairs on %s",
                     pos_part_name(mech, v1, v2).text);
      break;
    case REPAIRP_T:
      if (get_weapon_crits(mech, weapon_from_equipment_index(
                                     mech_critical_part_type(mech, v1, v2))) <
          5)
        extra_hard = 0;
      fix_bth =
          char_getskilltarget(mech_context(mech), player, "technician-weapons",
                              0) +
          REPLACE_DIFFICULTY +
          repair_weapon_type_difficulty(mech_critical_part_type(mech, v1, v2)) +
          extra_hard;
      fix_time = REPAIRGUN_TIME;
      (void)snprintf(buf, sizeof(buf), "Repairs on %s",
                     pos_part_name(mech, v1, v2).text);
      break;
    case REPAIRG:
      fix_bth =
          char_getskilltarget(mech_context(mech), player, "technician-weapons",
                              0) +
          REPLACE_DIFFICULTY +
          repair_weapon_type_difficulty(mech_critical_part_type(mech, v1, v2));
      fix_time =
          REPLACEGUN_TIME *
          clan_modified_time(
              mech, get_weapon_crits(
                        mech, weapon_from_equipment_index(
                                  mech_critical_part_type(mech, v1, v2))));
      (void)snprintf(buf, sizeof(buf), "Repairs on %s",
                     pos_part_name(mech, v1, v2).text);
      break;
    case ENHCRIT_MISC:
    case ENHCRIT_FOCUS:
    case ENHCRIT_CRYSTAL:
    case ENHCRIT_BARREL:
    case ENHCRIT_AMMOB:
    case ENHCRIT_RANGING:
    case ENHCRIT_AMMOM:
      fix_bth = char_getskilltarget(mech_context(mech), player,
                                    "technician-weapons", 0) +
                ENHCRIT_DIFFICULTY;
      fix_time = REPAIRENHCRIT_TIME;
      switch (damage->type) {
      case ENHCRIT_MISC:
        (void)snprintf(buf, sizeof(buf), "Repairs on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_FOCUS:
        (void)snprintf(buf, sizeof(buf), "Realign focus on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_CRYSTAL:
        (void)snprintf(buf, sizeof(buf), "Charging crystal repairs on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_BARREL:
        (void)snprintf(buf, sizeof(buf), "Barrel repairs on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_AMMOB:
      case ENHCRIT_AMMOM:
        (void)snprintf(buf, sizeof(buf), "Ammo feed repairs on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_RANGING:
        (void)snprintf(buf, sizeof(buf), "Ranging system repairs on %s",
                       pos_part_name(mech, v1, v2).text);
        break;
      default:
        break;
      }
      break;
    case SCRAPP:
      fix_bth = find_tech_skill(player, mech) + REMOVEP_DIFFICULTY;
      fix_time = REMOVEP_TIME;
      (void)snprintf(buf, sizeof(buf), "Removal of %s",
                     pos_part_name(mech, v1, v2).text);
      break;
    case SCRAPG:
      fix_bth = char_getskilltarget(mech_context(mech), player,
                                    "technician-weapons", 0) +
                REMOVEG_DIFFICULTY;
      fix_time =
          REMOVEG_TIME *
          clan_modified_time(
              mech, get_weapon_crits(
                        mech, weapon_from_equipment_index(
                                  mech_critical_part_type(mech, v1, v2))));
      (void)snprintf(buf, sizeof(buf), "Removal of %s",
                     pos_part_name(mech, v1, v2).text);
      break;
    case RELOAD:
      (void)snprintf(buf, sizeof(buf), "Reload of %s%s (%d rounds)",
                     pos_part_name(mech, v1, v2).text,
                     mech_critical_ammo_mode(mech, v1, v2)
                         ? get_ammo_desc_model_mode(
                               ammunition_to_weapon_index(
                                   mech_critical_part_type(mech, v1, v2)),
                               mech_critical_ammo_mode(mech, v1, v2))
                         : "",
                     mech_critical_full_ammunition(mech, v1, v2) -
                         mech_critical_data(mech, v1, v2));
      fix_time = RELOAD_TIME;
      fix_bth = find_tech_skill(player, mech) + RELOAD_DIFFICULTY;
      break;
    case UNLOAD:
      (void)snprintf(buf, sizeof(buf), "Unload of %s%s(%d rounds)",
                     pos_part_name(mech, v1, v2).text,
                     mech_critical_ammo_mode(mech, v1, v2)
                         ? get_ammo_desc_model_mode(
                               ammunition_to_weapon_index(
                                   mech_critical_part_type(mech, v1, v2)),
                               mech_critical_ammo_mode(mech, v1, v2))
                         : "",
                     mech_critical_data(mech, v1, v2));
      fix_time = RELOAD_TIME;
      fix_bth = find_tech_skill(player, mech) + REMOVES_DIFFICULTY;
      break;
    case FIXARMOR:
    case FIXARMOR_R:
    case FIXINTERNAL:
      const char *armor_material = "";
      if (damage->type == FIXINTERNAL) {
        if (mech_technology_flags(mech) & ES_TECH)
          armor_material = " Endosteel";
        else if (mech_technology_flags(mech) & REINFI_TECH)
          armor_material = " Reinforced";
        else if (mech_technology_flags(mech) & COMPI_TECH)
          armor_material = " Composite";
      } else if (mech_technology_flags(mech) & FF_TECH) {
        armor_material = " Ferrofibrous";
      } else if (mech_technology_flags(mech) & HARDA_TECH) {
        armor_material = " Hardened";
      } else if (mech_technology_flags_secondary(mech) & STEALTH_ARMOR_TECH) {
        armor_material = " Stealth";
      } else if (mech_technology_flags_secondary(mech) & HVY_FF_ARMOR_TECH) {
        armor_material = " Heavy Ferrofibrous";
      } else if (mech_technology_flags_secondary(mech) & LT_FF_ARMOR_TECH) {
        armor_material = " Light Ferrofibrous";
      } else if (mech_infantry_technology_flags(mech) &
                 CS_PURIFIER_STEALTH_TECH) {
        armor_material = " Purifier Stealth";
      }
      if (damage->type == FIXINTERNAL) {
        (void)snprintf(buf, sizeof(buf), "Repairs on%s internals (%d points)",
                       armor_material, damage->detail);
      } else if (damage->type == FIXARMOR_R) {
        (void)snprintf(buf, sizeof(buf), "Repairs on rear%s armor (%d points)",
                       armor_material, damage->detail);
      } else {
        (void)snprintf(buf, sizeof(buf), "Repairs on%s armor (%d points)",
                       armor_material, damage->detail);
      }
      fix_bth = find_tech_skill(player, mech) + (damage->type == FIXINTERNAL
                                                     ? FIXINTERNAL_DIFFICULTY
                                                     : FIXARMOR_DIFFICULTY);
      fix_time = damage->type == FIXINTERNAL ? FIXINTERNAL_TIME * damage->detail
                                             : FIXARMOR_TIME * damage->detail;
      break;
    }
    j = is_under_repair(damages, mech, i);
    if (j) {
      (void)snprintf(buf3, sizeof(buf3), "%4s %4s", "N/A", "N/A");
    } else {
      (void)snprintf(buf3, sizeof(buf3), "%4d %4d", fix_time, fix_bth);
    }
    (void)snprintf(
        buf2, sizeof(buf2), "[bold]%s%3s %3d %9s %3s %s[reset]%s",
        j ? "[fg=green]" : "[fg=yellow]", j ? "(*)" : "", i + 1, buf3,
        armor_section_abbreviation(
            &(ArmorSectionReference){.unit_class = mech_class(mech),
                                     .movement_type = mech_movement_type(mech),
                                     .location = v1})
            .text,
        buf, j ? " (*)" : "");
    cool_menu_add_text(&c, buf2);
  }
  cool_menu_add_line(&c);
  cool_menu_add_text(&c,
                     "(*) / [fg=green bold]Green[reset] = Job already done. "
                     "[fg=yellow bold]Yellow[reset] = To be done.");
  cool_menu_add_text(
      &c, "Time = Normal Time (in minutes) to complete fix. BTH = Your BTH to "
          "fix.");
  cool_menu_add_line(&c);
  show_cool_menu(btech_context_evaluation(mech_context(mech)), player, c);
  kill_cool_menu(c);
}
static void fix_entry(const RepairDamageTable *damages, DbRef player,
                      Mech *mech, int n) {
  char buf[MBUF_SIZE] = {0};
  char *c;
  n--;
  const RepairDamage *damage = repair_damage_const(damages, n);
  ArmorSectionAbbreviation abbreviation = armor_section_abbreviation(
      &(ArmorSectionReference){.unit_class = mech_class(mech),
                               .movement_type = mech_movement_type(mech),
                               .location = damage->location});
  c = abbreviation.text;
  switch (damage->type) {
  case REPAIRP_T:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_repairgun(player, mech, buf);
    break;
  case ENHCRIT_MISC:
  case ENHCRIT_FOCUS:
  case ENHCRIT_CRYSTAL:
  case ENHCRIT_BARREL:
  case ENHCRIT_AMMOB:
  case ENHCRIT_RANGING:
  case ENHCRIT_AMMOM:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_fixenhcrit(player, mech, buf);
    break;
  case REPAIRG:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_replacegun(player, mech, buf);
    break;
  case REPAIRP:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_replacepart(player, mech, buf);
    break;
  case RELOAD:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_reload(player, mech, buf);
    break;
  case REATTACH:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_reattach(player, mech, buf);
    break;
  case RESEAL:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_reseal(player, mech, buf);
    break;
  case FIXARMOR:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_fixarmor(player, mech, buf);
    break;
  case FIXARMOR_R:
    (void)snprintf(buf, sizeof(buf), "%s r", c);
    tech_fixarmor(player, mech, buf);
    break;
  case FIXINTERNAL:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_fixinternal(player, mech, buf);
    break;
  case DETACH:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_removesection(player, mech, buf);
    break;
  case SCRAPP:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_removepart(player, mech, buf);
    break;
  case SCRAPG:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_removegun(player, mech, buf);
    break;
  case UNLOAD:
    (void)snprintf(buf, sizeof(buf), "%s %d", c, damage->detail + 1);
    tech_unload(player, mech, buf);
    break;
  case REPLACESUIT:
    (void)snprintf(buf, sizeof(buf), "%s", c);
    tech_replacesuit(player, mech, buf);
    break;
  }
}
void tech_fix(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  RepairDamageTable damages_storage = {0};
  RepairDamageTable *damages = &damages_storage;
  int n;
  int low;
  int high = 0;
  RepairCommandContext repair_command;
  if (buffer != nullptr)
    buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(*buffer),
                                strspn(buffer, " \t\r\n\f\v"));
  char empty_buffer[] = "";
  if (!buffer)
    buffer = empty_buffer;
  RepairCommandStatus repair_status = repair_command_context_initialize(
      player, data, REPAIR_STALL_REQUIRED, &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  mech = repair_command.mech;
  if (unit_is_fixable(mech))
    make_damage_table(damages, mech);
  else
    make_scrap_table(damages, mech);
  if (!damages->count && mech_class(mech) == CLASS_MECH) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The 'mech is in pristine condition!");
    return;
  }
  if (!damages->count) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "It's in pristine condition!");
    return;
  }
  size_t range_offset = strcspn(buffer, "-");
  size_t buffer_length = strlen(buffer);
  if (range_offset < buffer_length) {
    char *range_separator = checked_storage_at(buffer, buffer_length + 1,
                                               sizeof(*buffer), range_offset);
    char *range_end = checked_storage_at(buffer, buffer_length + 1,
                                         sizeof(*buffer), range_offset + 1);
    *range_separator = '\0';
    bool valid_range = (parse_int_checked(buffer, &low) &&
                        parse_int_checked(range_end, &high)) != 0;
    *range_separator = '-';
    if (!valid_range) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid #!");
      return;
    }
    if (low < 1 || low > damages->count) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid low #!");
      return;
    }
    if (high < 1 || high > damages->count) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid high #!");
      return;
    }
    for (n = low; n <= high; n++)
      fix_entry(damages, player, mech, n);
    return;
  }
  if (!parse_int_checked(buffer, &n)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid #!");
    return;
  }
  if (n < 1 || n > damages->count) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid #!");
    return;
  }
  fix_entry(damages, player, mech, n);
}
