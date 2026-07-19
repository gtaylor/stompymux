
/*
 * $Id: mech.tech.damages.c,v 1.1.1.1 2005/01/11 21:18:25 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Mon Dec  2 19:00:35 1996 fingon
 * Last modified: Thu Sep 10 09:55:13 1998 fingon
 *
 */

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "coolmenu.h"
#include "failures.h"
#include "mech.events.h"
#include "mech.h"
#include "mech.tech.damages.h"
#include "mech.tech.h"
#include "mycool.h"
#include "p.btechstats.h"
#include "p.mech.build.h"
#include "p.mech.status.h"
#include "p.mech.tech.commands.h"
#include "p.mech.utils.h"

typedef struct RepairDamageTable {
  /* Each entry stores type, location, and position or amount. */
  short entries[MAX_DAMAGES][3];
  int count;
} RepairDamageTable;

static void append_damage(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));

static void append_damage(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
  vsnprintf(buffer + len, size - len, fmt, ap);
  va_end(ap);
}

static const char *const repair_need_msgs[] = {
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

#define CHECK(loc) check_for_damage(damages, mech, loc)
#define DAMAGE2(a, b)                                                          \
  do {                                                                         \
    damages->entries[damages->count][0] = a;                                   \
    damages->entries[damages->count++][1] = b;                                 \
  } while (0)
#define DAMAGE3(a, b, c)                                                       \
  do {                                                                         \
    damages->entries[damages->count][0] = a;                                   \
    damages->entries[damages->count][1] = b;                                   \
    damages->entries[damages->count++][2] = c;                                 \
  } while (0)

#define ClanMod(num)                                                           \
  MAX(1, (((num) / ((MechSpecials(mech) & CLAN_TECH) ? 2 : 1))))

static int check_for_damage(RepairDamageTable *damages, MECH *mech, int loc) {
  int a, b, c, d;

  if (SectIsDestroyed(mech, loc)) {
    if (MechType(mech) != CLASS_BSUIT)
      DAMAGE2(REATTACH, loc);
    else
      DAMAGE2(REPLACESUIT, loc);
    return 0;
  }

  /*
   * Added by Kipsta
   * 8/4/99
   */

  if (SectIsFlooded(mech, loc)) {
    DAMAGE2(RESEAL, loc);
    return 0;
  }
  if ((a = GetSectInt(mech, loc)) != (b = GetSectOInt(mech, loc)))
    DAMAGE3(FIXINTERNAL, loc, (b - a));
  else {
    if ((a = GetSectArmor(mech, loc)) != (b = GetSectOArmor(mech, loc)))
      DAMAGE3(FIXARMOR, loc, (b - a));
    if ((a = GetSectRArmor(mech, loc)) != (b = GetSectORArmor(mech, loc)))
      DAMAGE3(FIXARMOR_R, loc, (b - a));
  }
  for (a = 0; a < NUM_CRITICALS; a++) {
    if (!(b = GetPartType(mech, loc, a)))
      continue;
    if (IsAmmo(b) && !PartIsDestroyed(mech, loc, a) &&
        (c = GetPartData(mech, loc, a)) != (d = FullAmmo(mech, loc, a)))
      DAMAGE3(RELOAD, loc, a);
    if (!PartIsNonfunctional(mech, loc, a) && !PartTempNuke(mech, loc, a) &&
        !PartIsDamaged(mech, loc, a))
      continue;
    if (IsCrap(b))
      continue;
    /* Destroyed / tempnuke'd part. Either case, it works for us :) */

    if (PartIsDamaged(mech, loc, a)) {
      if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_EN_FOCUS)
        DAMAGE3(ENHCRIT_FOCUS, loc, a);
      else if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_EN_CRYSTAL)
        DAMAGE3(ENHCRIT_CRYSTAL, loc, a);
      else if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_BALL_BARREL)
        DAMAGE3(ENHCRIT_BARREL, loc, a);
      else if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_BALL_AMMO)
        DAMAGE3(ENHCRIT_AMMOB, loc, a);
      else if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_MSL_RANGING)
        DAMAGE3(ENHCRIT_RANGING, loc, a);
      else if (GetPartDamageFlags(mech, loc, a) & WEAP_DAM_MSL_AMMO)
        DAMAGE3(ENHCRIT_AMMOM, loc, a);
      else
        DAMAGE3(ENHCRIT_MISC, loc, a);

    } else if (IsWeapon(b) && !PartIsDestroyed(mech, loc, a))
      DAMAGE3(REPAIRP_T, loc, a);
    else
      DAMAGE3(IsWeapon(b) ? REPAIRG : REPAIRP, loc, a);

    if (IsWeapon(b))
      a += GetWeaponCrits(mech, Weapon2I(b)) - 1;
  }
  return 1;
}

static int check_for_scrappage(RepairDamageTable *damages, MECH *mech,
                               int loc) {
  int a, b;
  int ret = 1;

  if (SectIsDestroyed(mech, loc))
    return 1;

  if (SomeoneScrappingLoc(mech, loc)) {
    DAMAGE2(DETACH, loc);
    return 1;
  }
  for (a = 0; a < NUM_CRITICALS; a++) {
    if (!(b = GetPartType(mech, loc, a)))
      continue;
    if (PartIsBroken(mech, loc, a))
      continue;
    if (IsCrap(b))
      continue;
    if (IsAmmo(b) && GetPartData(mech, loc, a)) {
      DAMAGE3(UNLOAD, loc, a);
      if (ret && !SomeoneRepairing(mech, loc, a))
        ret = 0;
      continue;
    }
    DAMAGE3(IsWeapon(b) ? SCRAPG : SCRAPP, loc, a);
    if (ret && !SomeoneScrappingPart(mech, loc, a))
      ret = 0;
    if (IsWeapon(b))
      a += GetWeaponCrits(mech, Weapon2I(b)) - 1;
  }

  if (ret && !Invalid_Scrap_Path(mech, loc))
    DAMAGE2(DETACH, loc);

  return 0;
}

static void make_scrap_table(RepairDamageTable *damages, MECH *mech) {
  int i = 4;

  damages->count = 0;
  if (MechType(mech) == CLASS_MECH) {
    if (check_for_scrappage(damages, mech, RARM))
      i -= check_for_scrappage(damages, mech, RTORSO);
    if (check_for_scrappage(damages, mech, LARM))
      i -= check_for_scrappage(damages, mech, LTORSO);
    i -= check_for_scrappage(damages, mech, RLEG);
    i -= check_for_scrappage(damages, mech, LLEG);

    if (!i)
      check_for_scrappage(damages, mech, CTORSO);

    check_for_scrappage(damages, mech, HEAD);
  } else
    for (i = 0; i < NUM_SECTIONS; i++)
      if (GetSectOInt(mech, i))
        check_for_scrappage(damages, mech, i);
}

static void make_damage_table(RepairDamageTable *damages, MECH *mech) {
  int i;

  damages->count = 0;
  if (MechType(mech) == CLASS_MECH) {
    if (check_for_damage(damages, mech, CTORSO)) {
      if (check_for_damage(damages, mech, LTORSO)) {
        CHECK(LARM);
      }
      if (check_for_damage(damages, mech, RTORSO)) {
        CHECK(RARM);
      }
      CHECK(LLEG);
      CHECK(RLEG);
      CHECK(HEAD);
    }
  } else
    for (i = 0; i < NUM_SECTIONS; i++)
      if (GetSectOInt(mech, i))
        check_for_damage(damages, mech, i);
}

static int is_under_repair(const RepairDamageTable *damages, MECH *mech,
                           int i) {
  int v1 = damages->entries[i][1];
  int v2 = damages->entries[i][2];

  switch (damages->entries[i][0]) {
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
    return SomeoneRepairing(mech, v1, v2);
  case REATTACH:
    return SomeoneAttaching(mech, v1);
  case RESEAL:
    return SomeoneResealing(mech, v1);
  case FIXARMOR_R:
    return SomeoneFixing(mech, v1 + 8);
  case FIXARMOR:
  case FIXINTERNAL:
    return SomeoneFixing(mech, v1);
  case DETACH:
    return SomeoneScrappingLoc(mech, v1);
  case SCRAPP:
  case SCRAPG:
    return SomeoneScrappingPart(mech, v1, v2);
  case REPLACESUIT:
    return SomeoneReplacingSuit(mech, v1);
  }
  return 0;
}

void mech_repair_jobs_format(MECH *mech, char *buffer, size_t buffer_size) {
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
    /* Ok... i think we want: */
    /* repairnum|location|typenum|data|fixing? */
    if (i)
      append_damage(buffer, buffer_size, ",");
    append_damage(buffer, buffer_size, "%d|%s|%d|", i + 1,
                  armor_section_abbreviation(MechType(mech), MechMove(mech),
                                             damages->entries[i][1])
                      .text,
                  (int)damages->entries[i][0]);
    switch (damages->entries[i][0]) {
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
      append_damage(
          buffer, buffer_size, "%s",
          pos_part_name(mech, damages->entries[i][1], damages->entries[i][2])
              .text);
      break;
    case RELOAD:
      append_damage(
          buffer, buffer_size, "%s:%d",
          pos_part_name(mech, damages->entries[i][1], damages->entries[i][2])
              .text,
          FullAmmo(mech, damages->entries[i][1], damages->entries[i][2]) -
              GetPartData(mech, damages->entries[i][1],
                          damages->entries[i][2]));
      break;
    case UNLOAD:
      append_damage(
          buffer, buffer_size, "%s:%d",
          pos_part_name(mech, damages->entries[i][1], damages->entries[i][2])
              .text,
          GetPartData(mech, damages->entries[i][1], damages->entries[i][2]));
      break;
    case FIXARMOR:
    case FIXARMOR_R:
    case FIXINTERNAL:
      append_damage(buffer, buffer_size, "%d", damages->entries[i][2]);
      break;
    default:
      append_damage(buffer, buffer_size, "-");
    }
    append_damage(buffer, buffer_size, "|%d",
                  is_under_repair(damages, mech, i));
  }
}

size_t mech_repair_job_count(MECH *mech) {
  RepairDamageTable damages = {0};

  if (unit_is_fixable(mech))
    make_damage_table(&damages, mech);
  else
    make_scrap_table(&damages, mech);
  return (size_t)damages.count;
}

void show_mechs_damage(DbRef player, void *data, char *buffer) {
  MECH *mech = data;
  RepairDamageTable damages_storage = {0};
  RepairDamageTable *damages = &damages_storage;
  coolmenu *c = NULL;
  int i, j, v1, v2;
  char buf[MBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  char buf3[MBUF_SIZE] = {0};
  int isds;
  int fix_time = 0;
  int fix_bth = 0;
  int extra_hard = 1;

  TECHCOMMANDD;
  if (unit_is_fixable(mech))
    make_damage_table(damages, mech);
  else
    make_scrap_table(damages, mech);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !damages->count && MechType(mech) == CLASS_MECH,
                  "The 'mech is in pristine condition!");
  DOCHECK_CONTEXT(mech->xcode.context, !damages->count,
                  "It's in pristine condition!");
  addline();
  cent(tprintf("Damage for %s", mech_display_id(mech).text));
  addline();
  vsi("   Fix# Time  BTH Loc Description");
  for (i = 0; i < damages->count; i++) {
    v1 = damages->entries[i][1];
    v2 = damages->entries[i][2];
    switch (damages->entries[i][0]) {
    case REATTACH:
      fix_bth = FindTechSkill(player, mech) + REATTACH_DIFFICULTY;
      fix_time = REATTACH_TIME;
      strcpy(buf, repair_need_msgs[(int)damages->entries[i][0]]);
      break;
    case DETACH:
      fix_bth = FindTechSkill(player, mech) + REMOVES_DIFFICULTY;
      fix_time = REMOVES_TIME;
      strcpy(buf, repair_need_msgs[(int)damages->entries[i][0]]);
      break;
    case RESEAL:
      fix_bth = FindTechSkill(player, mech) + RESEAL_DIFFICULTY;
      fix_time = RESEAL_TIME;
      strcpy(buf, repair_need_msgs[(int)damages->entries[i][0]]);
      break;
    case REPLACESUIT:
      strcpy(buf, repair_need_msgs[(int)damages->entries[i][0]]);
      fix_time = REPLACESUIT_TIME;
      fix_bth = FindTechSkill(player, mech) + REPLACESUIT_DIFFICULTY;
      break;
    case REPAIRP:
      fix_bth = FindTechSkill(player, mech) + REPLACE_DIFFICULTY +
                PARTTYPE_DIFFICULTY(GetPartType(mech, v1, v2));
      fix_time = REPLACEPART_TIME;
      snprintf(buf, sizeof(buf), "Repairs on %s",
               pos_part_name(mech, v1, v2).text);
      break;
    case REPAIRP_T:
      if (GetWeaponCrits(mech, Weapon2I(GetPartType(mech, v1, v2))) < 5)
        extra_hard = 0;
      fix_bth = char_getskilltarget(mech->xcode.context, player,
                                    "technician-weapons", 0) +
                REPLACE_DIFFICULTY +
                WEAPTYPE_DIFFICULTY(GetPartType(mech, v1, v2)) + extra_hard;
      fix_time = REPAIRGUN_TIME;
      snprintf(buf, sizeof(buf), "Repairs on %s",
               pos_part_name(mech, v1, v2).text);
      break;
    case REPAIRG:
      fix_bth = char_getskilltarget(mech->xcode.context, player,
                                    "technician-weapons", 0) +
                REPLACE_DIFFICULTY +
                WEAPTYPE_DIFFICULTY(GetPartType(mech, v1, v2));
      fix_time =
          REPLACEGUN_TIME *
          ClanMod(GetWeaponCrits(mech, Weapon2I(GetPartType(mech, v1, v2))));
      snprintf(buf, sizeof(buf), "Repairs on %s",
               pos_part_name(mech, v1, v2).text);
      break;
    case ENHCRIT_MISC:
    case ENHCRIT_FOCUS:
    case ENHCRIT_CRYSTAL:
    case ENHCRIT_BARREL:
    case ENHCRIT_AMMOB:
    case ENHCRIT_RANGING:
    case ENHCRIT_AMMOM:
      fix_bth = char_getskilltarget(mech->xcode.context, player,
                                    "technician-weapons", 0) +
                ENHCRIT_DIFFICULTY;
      fix_time = REPAIRENHCRIT_TIME;
      switch (damages->entries[i][0]) {
      case ENHCRIT_MISC:
        snprintf(buf, sizeof(buf), "Repairs on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_FOCUS:
        snprintf(buf, sizeof(buf), "Realign focus on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_CRYSTAL:
        snprintf(buf, sizeof(buf), "Charging crystal repairs on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_BARREL:
        snprintf(buf, sizeof(buf), "Barrel repairs on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_AMMOB:
      case ENHCRIT_AMMOM:
        snprintf(buf, sizeof(buf), "Ammo feed repairs on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      case ENHCRIT_RANGING:
        snprintf(buf, sizeof(buf), "Ranging system repairs on %s",
                 pos_part_name(mech, v1, v2).text);
        break;
      default:
        break;
      }
      break;
    case SCRAPP:
      fix_bth = FindTechSkill(player, mech) + REMOVEP_DIFFICULTY;
      fix_time = REMOVEP_TIME;
      snprintf(buf, sizeof(buf), "Removal of %s",
               pos_part_name(mech, v1, v2).text);
      break;
    case SCRAPG:
      fix_bth = char_getskilltarget(mech->xcode.context, player,
                                    "technician-weapons", 0) +
                REMOVEG_DIFFICULTY;
      fix_time =
          REMOVEG_TIME *
          ClanMod(GetWeaponCrits(mech, Weapon2I(GetPartType(mech, v1, v2))));
      snprintf(buf, sizeof(buf), "Removal of %s",
               pos_part_name(mech, v1, v2).text);
      break;
    case RELOAD:
      snprintf(
          buf, sizeof(buf), "Reload of %s%s (%d rounds)",
          pos_part_name(mech, v1, v2).text,
          GetPartAmmoMode(mech, v1, v2)
              ? GetAmmoDesc_Model_Mode(Ammo2WeaponI(GetPartType(mech, v1, v2)),
                                       GetPartAmmoMode(mech, v1, v2))
              : "",
          FullAmmo(mech, v1, v2) - GetPartData(mech, v1, v2));
      fix_time = RELOAD_TIME;
      fix_bth = FindTechSkill(player, mech) + RELOAD_DIFFICULTY;
      break;
    case UNLOAD:
      snprintf(
          buf, sizeof(buf), "Unload of %s%s(%d rounds)",
          pos_part_name(mech, v1, v2).text,
          GetPartAmmoMode(mech, v1, v2)
              ? GetAmmoDesc_Model_Mode(Ammo2WeaponI(GetPartType(mech, v1, v2)),
                                       GetPartAmmoMode(mech, v1, v2))
              : "",
          GetPartData(mech, v1, v2));
      fix_time = RELOAD_TIME;
      fix_bth = FindTechSkill(player, mech) + REMOVES_DIFFICULTY;
      break;
    case FIXARMOR:
    case FIXARMOR_R:
    case FIXINTERNAL:
      const char *armor_material =
          damages->entries[i][0] == FIXINTERNAL
              ? ((MechSpecials(mech) & ES_TECH)       ? " Endosteel"
                 : (MechSpecials(mech) & REINFI_TECH) ? " Reinforced"
                 : (MechSpecials(mech) & COMPI_TECH)  ? " Composite"
                                                      : "")
              : ((MechSpecials(mech) & FF_TECH)               ? " Ferrofibrous"
                 : (MechSpecials(mech) & HARDA_TECH)          ? " Hardened"
                 : (MechSpecials2(mech) & STEALTH_ARMOR_TECH) ? " Stealth"
                 : (MechSpecials2(mech) & HVY_FF_ARMOR_TECH)
                     ? " Heavy Ferrofibrous"
                 : (MechSpecials2(mech) & LT_FF_ARMOR_TECH)
                     ? " Light Ferrofibrous"
                 : (MechInfantrySpecials(mech) & CS_PURIFIER_STEALTH_TECH)
                     ? " Purifier Stealth"
                     : "");
      if (damages->entries[i][0] == FIXINTERNAL) {
        snprintf(buf, sizeof(buf), "Repairs on%s internals (%d points)",
                 armor_material, damages->entries[i][2]);
      } else if (damages->entries[i][0] == FIXARMOR_R) {
        snprintf(buf, sizeof(buf), "Repairs on rear%s armor (%d points)",
                 armor_material, damages->entries[i][2]);
      } else {
        snprintf(buf, sizeof(buf), "Repairs on%s armor (%d points)",
                 armor_material, damages->entries[i][2]);
      }
      fix_bth = FindTechSkill(player, mech) +
                (damages->entries[i][0] == FIXINTERNAL ? FIXINTERNAL_DIFFICULTY
                                                       : FIXARMOR_DIFFICULTY);
      fix_time = damages->entries[i][0] == FIXINTERNAL
                     ? FIXINTERNAL_TIME * damages->entries[i][2]
                     : FIXARMOR_TIME * damages->entries[i][2];
      break;
    }
    j = is_under_repair(damages, mech, i);
    if (j) {
      snprintf(buf3, sizeof(buf3), "%4s %4s", "N/A", "N/A");
    } else {
      snprintf(buf3, sizeof(buf3), "%4d %4d", fix_time, fix_bth);
    }
    snprintf(
        buf2, sizeof(buf2), "%%ch%s%3s %3d %9s %3s %s%%cn%s", j ? "%cg" : "%cy",
        j ? "(*)" : "", i + 1, buf3,
        armor_section_abbreviation(MechType(mech), MechMove(mech), v1).text,
        buf, j ? " (*)" : "");
    vsi(buf2);
  }
  addline();
  vsi("(*) / %ch%cgGreen%cn = Job already done. %ch%cyYellow%cn = To be done.");
  vsi("Time = Normal Time (in minutes) to complete fix. BTH = Your BTH to "
      "fix.");
  addline();
  ShowCoolMenu(btech_context_evaluation(mech->xcode.context), player, c);
  KillCoolMenu(c);
}

static void fix_entry(const RepairDamageTable *damages, DbRef player,
                      MECH *mech, int n) {
  char buf[MBUF_SIZE] = {0};
  char *c;

  /* whee */
  n--;
  ArmorSectionAbbreviation abbreviation = armor_section_abbreviation(
      MechType(mech), MechMove(mech), damages->entries[n][1]);
  c = abbreviation.text;
  switch (damages->entries[n][0]) {
  case REPAIRP_T:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_repairgun(player, mech, buf);
    break;
  case ENHCRIT_MISC:
  case ENHCRIT_FOCUS:
  case ENHCRIT_CRYSTAL:
  case ENHCRIT_BARREL:
  case ENHCRIT_AMMOB:
  case ENHCRIT_RANGING:
  case ENHCRIT_AMMOM:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_fixenhcrit(player, mech, buf);
    break;
  case REPAIRG:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_replacegun(player, mech, buf);
    break;
  case REPAIRP:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_replacepart(player, mech, buf);
    break;
  case RELOAD:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_reload(player, mech, buf);
    break;
  case REATTACH:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_reattach(player, mech, buf);
    break;
  case RESEAL:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_reseal(player, mech, buf);
    break;
  case FIXARMOR:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_fixarmor(player, mech, buf);
    break;
  case FIXARMOR_R:
    snprintf(buf, sizeof(buf), "%s r", c);
    tech_fixarmor(player, mech, buf);
    break;
  case FIXINTERNAL:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_fixinternal(player, mech, buf);
    break;
  case DETACH:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_removesection(player, mech, buf);
    break;
  case SCRAPP:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_removepart(player, mech, buf);
    break;
  case SCRAPG:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_removegun(player, mech, buf);
    break;
  case UNLOAD:
    snprintf(buf, sizeof(buf), "%s %d", c, damages->entries[n][2] + 1);
    tech_unload(player, mech, buf);
    break;
  case REPLACESUIT:
    snprintf(buf, sizeof(buf), "%s", c);
    tech_replacesuit(player, mech, buf);
    break;
  }
}

void tech_fix(DbRef player, void *data, char *buffer) {
  MECH *mech = data;
  RepairDamageTable damages_storage = {0};
  RepairDamageTable *damages = &damages_storage;
  int n = atoi(buffer);
  int low, high;
  int isds;

  skipws(buffer);
  TECHCOMMANDC;
  if (unit_is_fixable(mech))
    make_damage_table(damages, mech);
  else
    make_scrap_table(damages, mech);
  DOCHECK_CONTEXT(mech->xcode.context,
                  !damages->count && MechType(mech) == CLASS_MECH,
                  "The 'mech is in pristine condition!");
  DOCHECK_CONTEXT(mech->xcode.context, !damages->count,
                  "It's in pristine condition!");
  if (sscanf(buffer, "%d-%d", &low, &high) == 2) {
    DOCHECK_CONTEXT(mech->xcode.context, low < 1 || low > damages->count,
                    "Invalid low #!");
    DOCHECK_CONTEXT(mech->xcode.context, high < 1 || high > damages->count,
                    "Invalid high #!");
    for (n = low; n <= high; n++)
      fix_entry(damages, player, mech, n);
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context, n < 1 || n > damages->count,
                  "Invalid #!");
  fix_entry(damages, player, mech, n);
}
