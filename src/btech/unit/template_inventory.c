#include "mech_equipment_api.h"
#include "mech_status_types.h"
#include "template_internal.h"
#include "weapon_catalogue_api.h"

void DumpMechSpecialObjects(BtechContext *context, DbRef player) {
  CoolMenu *c;

  c = auto_column_string_menu("MechSpecials available", internals);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

static char *dumpweapon_fun(void *data, int i, char buffer[static LBUF_SIZE]) {
  const BtechWeaponSettings *weapon_settings = data;

  buffer[0] = 0;
  if (!i)
    snprintf(buffer, LBUF_SIZE, WDUMP_MASKS);
  else {
    i--;
    snprintf(buffer, LBUF_SIZE, WDUMP_MASK, MechWeapons[i].name,
             MechWeapons[i].heat, MechWeapons[i].damage, MechWeapons[i].min,
             MechWeapons[i].shortrange, MechWeapons[i].medrange,
             weapon_catalogue_effective_range(i, false),
             btech_weapon_settings_recycle_time(weapon_settings, i),
             MechWeapons[i].criticals, MechWeapons[i].ammoperton);
  }
  return buffer;
}

void DumpWeapons(BtechContext *context, DbRef player) {
  CoolMenu *c;

  c = SelCol_FunStringMenuContextK(1, "MechWeapons available", dumpweapon_fun,
                                   &context->weapon_settings,
                                   num_def_weapons + 1);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

char *techlist_func(Mech *mech, char *buffer) {
  char bufa[SBUF_SIZE], bufb[SBUF_SIZE], bufc[SBUF_SIZE];
  int i, ii, part = 0, axe = 0, mace = 0, sword = 0, saw = 0, claw = 0,
             hascase = 0;

  snprintf(bufa, SBUF_SIZE, "%s",
           build_bit_string(specialsabrev, ((mech)->rd.specials),
                            (char[BTECH_TEXT_CAPACITY]){0}));
  snprintf(bufb, SBUF_SIZE, "%s",
           build_bit_string(specialsabrev2, ((mech)->rd.specials2),
                            (char[BTECH_TEXT_CAPACITY]){0}));
  snprintf(buffer, MBUF_SIZE, "%s %s", bufa, bufb);

  if (((mech)->ud.type) == CLASS_BSUIT) {
    snprintf(bufc, SBUF_SIZE, "%s",
             build_bit_string(infspecialsabrev, ((mech)->rd.infantry_specials),
                              (char[BTECH_TEXT_CAPACITY]){0}));
    snprintf(buffer, MBUF_SIZE, "%s %s %s", bufa, bufb, bufc);
  } else
    snprintf(buffer, MBUF_SIZE, "%s %s", bufa, bufb);

  if (!(strstr(buffer, "XL") || strstr(buffer, "XXL") ||
        strstr(buffer, "LENG") || strstr(buffer, "ICE") ||
        strstr(buffer, "CENG")) &&
      (((mech)->ud.type) != CLASS_BSUIT))
    strcat(buffer, " FUS ");

  for (i = 0; i < NUM_SECTIONS; i++)
    for (ii = 0; ii < NUM_CRITICALS; ii++) {
      part = mech_critical_part_type(mech, i, ii);
      if (part == special_equipment_index(AXE) && !axe) {
        axe = 1;
        strcat(buffer, " AXE");
      }
      if (part == special_equipment_index(CLAW) && !claw) {
        claw = 1;
        strcat(buffer, " CLAW");
      }
      if (part == special_equipment_index(MACE) && !mace) {
        mace = 1;
        strcat(buffer, " MACE");
      }
      if (part == special_equipment_index(DUAL_SAW) && !saw) {
        saw = 1;
        strcat(buffer, " DUAL_SAW");
      }
      if (part == special_equipment_index(SWORD) && !sword) {
        sword = 1;
        strcat(buffer, " SWORD");
      }
      if ((((mech)->ud.sections)[i].config & CASE_TECH) && !hascase) {
        hascase = 1;
        strcat(buffer, " CASE");
      }
    }

  if (((mech)->ud.cargospace))
    strcat(buffer, " INFC");

  if (((mech)->ud.type) == CLASS_VTOL)
    strcat(buffer, " VTOL");

  if (((mech)->ud.type) == CLASS_MECH && ((mech)->ud.move) != MOVE_QUAD) {
    if ((mech_critical_is_operational_special(mech, RARM, 3,
                                              HAND_OR_FOOT_ACTUATOR) &&
         mech_critical_is_operational_special(mech, RARM, 0,
                                              SHOULDER_OR_HIP)) ||
        (mech_critical_is_operational_special(mech, LARM, 3,
                                              HAND_OR_FOOT_ACTUATOR) &&
         mech_critical_is_operational_special(mech, LARM, 0,
                                              SHOULDER_OR_HIP)) ||
        ((mech)->rd.specials) & SALVAGE_TECH)
      strcat(buffer, " MTOW");
  } else {
    if (((mech)->rd.specials) & SALVAGE_TECH)
      strcat(buffer, " MTOW");
  }

  return buffer;
}

/* Function to return the payload of a unit
 * Used by the btpayload_ref() scode function
 * Dany - 06/2005 */
char *payloadlist_func(Mech *mech, char *buffer) {

  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int temp_crit;

  int count, weap_count, ammo_count, section_loop, weap_loop, put_loop;
  char payloadbuff[120] = {0};

  short payload_items[8 * MAX_WEAPS_SECTION];
  short payload_items_count[8 * MAX_WEAPS_SECTION];

  /* Clear the buffer */
  snprintf(buffer, MBUF_SIZE, "%s", "");

  /* Count each 'unique' item */
  weap_count = 0;
  ammo_count = 0;

  /* Initialize array */
  for (put_loop = 0; put_loop < 8 * MAX_WEAPS_SECTION; put_loop++) {
    payload_items[put_loop] = -1;
    payload_items_count[put_loop] = 0;
  }

  /* Get the weapons for each sections */
  for (section_loop = 0; section_loop < NUM_SECTIONS; section_loop++) {

    /* Get all the weapons for that section */
    count = FindWeapons_Advanced(mech, section_loop, weaparray, weapdata,
                                 critical, 1);
    /* Check if any weapons in that section */
    if (count <= 0)
      continue;

    /* Loop through all the weapons found and store their values */
    for (weap_loop = 0; weap_loop < count; weap_loop++) {
      if (!(mech_critical_is_broken(mech, section_loop, critical[weap_loop]))) {
        /* Loop to put weapons in the temp array and keep count */
        for (put_loop = 0; put_loop < 8 * MAX_WEAPS_SECTION; put_loop++) {

          /* Check to see if there is already an entry */
          if (payload_items[put_loop] == weaparray[weap_loop]) {
            payload_items_count[put_loop]++;
            break;
            /* Ok, see if there is no entry */
          } else if (payload_items[put_loop] == -1) {
            payload_items[put_loop] = weaparray[weap_loop];
            payload_items_count[put_loop]++;
            weap_count++;
            break;
          }

        } /* End of put loop */
      } /* end destroyed check */
    } /* End of weap count loop */

  } /* End of section loop */

  /* Loop through all the sections */
  for (section_loop = 0; section_loop < NUM_SECTIONS; section_loop++) {

    /* Loop through all the crits in a section */
    for (count = 0; count < MAX_WEAPS_SECTION; count++) {

      /* Get the Part at that spot */
      temp_crit = mech_critical_part_type(mech, section_loop, count);

      /* Is it Ammo? */
      if (equipment_is_ammunition(temp_crit)) {
        if (!mech_critical_is_destroyed(mech, section_loop, count)) {
          /* Loop to put weapons in the temp array and keep count */
          for (put_loop = weap_count; put_loop < 8 * MAX_WEAPS_SECTION;
               put_loop++) {

            /* Check to see if there is already an entry */
            if (payload_items[put_loop] == temp_crit) {
              payload_items_count[put_loop]++;
              break;
              /* Ok, see if there is no entry */
            } else if (payload_items[put_loop] == -1) {
              payload_items[put_loop] = temp_crit;
              payload_items_count[put_loop]++;
              ammo_count++;
              break;
            }

          } /* End of put loop */
        } /*end part is destroyed check */
      }
      /* End of is it Ammo if Statement */

    } /* End of Crit Slot Loop */

  } /* End of Section Loop */

  /* Final loop to print out the full payload to the buffer and return it */
  for (put_loop = 0; put_loop < (weap_count + ammo_count); put_loop++) {

    /* If its a weapon use this method of printing it out
     * Else use the part method */
    if (put_loop < weap_count) {
      snprintf(payloadbuff, sizeof(payloadbuff), "%s:%d",
               &MechWeapons[payload_items[put_loop]].name[0],
               payload_items_count[put_loop]);
    } else {
      snprintf(payloadbuff, sizeof(payloadbuff), "%s:%d",
               partname_func(mech->xcode.context, payload_items[put_loop], 'V'),
               payload_items_count[put_loop]);
    }

    /* If we are not at the end, then put a | as a spacer */
    if (put_loop < (weap_count + ammo_count - 1)) {
      strncat(payloadbuff, "|", sizeof(buffer) - strlen(buffer) - 1);
    }

    /* Adding it to the main buffer */
    strncat(buffer, payloadbuff, sizeof(buffer) - strlen(buffer) - 1);

  } /* End of printing loop */

  return buffer;
}

// Borrowed from payload_func
char *partlist_func(Mech *mech, char *buffer) {

  int temp_crit;

  int count, part_count, section_loop, put_loop, act_count;
  char partlistbuff[120] = {0};

  short partlist_items[8 * MAX_WEAPS_SECTION];
  short partlist_count[8 * MAX_WEAPS_SECTION];

  /* Clear the buffer */
  snprintf(buffer, LBUF_SIZE, "%s", "");

  /* Count each 'unique' item */
  part_count = 0;
  act_count = 0;

  /* Initialize array */
  for (put_loop = 0; put_loop < 8 * MAX_WEAPS_SECTION; put_loop++) {
    partlist_items[put_loop] = -1;
    partlist_count[put_loop] = 0;
  }

  /* Get the parts for each sections */
  for (section_loop = 0; section_loop < NUM_SECTIONS; section_loop++) {

    /* Loop through all the crits in a section */
    for (count = 0; count < MAX_WEAPS_SECTION; count++) {

      /* Get the Part at that spot */
      temp_crit = mech_critical_part_type(mech, section_loop, count);
      if (mech_critical_is_destroyed(mech, section_loop, count))
        continue;

      /*	Things we don't need */
      if (equipment_is_ammunition(temp_crit))
        continue;
      if (equipment_is_weapon(temp_crit))
        continue;
      if (temp_crit < 1)
        continue;

      switch (special_from_equipment_index(temp_crit)) {
      case ENDO_STEEL:
      case LT_FERRO_FIBROUS:
      case HVY_FERRO_FIBROUS:
      case FERRO_FIBROUS:
        continue;
      default:
        break;
      }
      /* Loop to put parts in the temp array and keep count */
      for (put_loop = 0; put_loop < 8 * MAX_WEAPS_SECTION; put_loop++) {

        /* Check to see if there is already an entry */
        if (partlist_items[put_loop] == temp_crit) {
          partlist_count[put_loop]++;
          break;
          /* Ok, see if there is no entry */
        } else if (partlist_items[put_loop] == -1) {
          partlist_items[put_loop] = temp_crit;
          partlist_count[put_loop]++;
          part_count++;
          break;
        }

      } /* End of put loop */

    } /* End of Crit Slot Loop */

  } /* End of Section Loop */

  /* Final loop to print out the full part list to the buffer and return it */
  for (put_loop = 0; put_loop < (part_count); put_loop++) {

    switch (special_from_equipment_index(partlist_items[put_loop])) {
    case LOWER_ACTUATOR:
    case UPPER_ACTUATOR:
    case SHOULDER_OR_HIP:
    case HAND_OR_FOOT_ACTUATOR:
      act_count = act_count + partlist_count[put_loop];
      break;
    case ENGINE:
      snprintf(partlistbuff, sizeof(partlistbuff), "%s:%d",
               ((mech)->rd.specials) & LE_TECH    ? "Light_Engine"
               : ((mech)->rd.specials) & CE_TECH  ? "Compact_Engine"
               : ((mech)->rd.specials) & XXL_TECH ? "XXL_Engine"
               : ((mech)->rd.specials) & XL_TECH  ? "XL_Engine"
               : ((mech)->rd.specials) & ICE_TECH ? "ICE_Engine"
                                                  : "Engine",
               partlist_count[put_loop]);

      /* If we are not at the end, then put a | as a spacer */
      if (put_loop < (part_count - 1)) {
        strncat(partlistbuff, "|", sizeof(buffer) - strlen(buffer) - 1);
      }

      /* Adding it to the main buffer */
      strncat(buffer, partlistbuff, sizeof(buffer) - strlen(buffer) - 1);
      break;
    case GYRO:
      snprintf(partlistbuff, sizeof(partlistbuff), "%s:%d",
               ((mech)->rd.specials2) & XLGYRO_TECH   ? "XL_Gyro"
               : ((mech)->rd.specials2) & HDGYRO_TECH ? "HeavyDuty_Gyro"
                                                      : "Gyro",
               partlist_count[put_loop]);

      /* If we are not at the end, then put a | as a spacer */
      if (put_loop < (part_count - 1)) {
        strncat(partlistbuff, "|", sizeof(buffer) - strlen(buffer) - 1);
      }

      /* Adding it to the main buffer */
      strncat(buffer, partlistbuff, sizeof(buffer) - strlen(buffer) - 1);
      break;
    case HEAT_SINK:
      snprintf(partlistbuff, sizeof(partlistbuff), "%s:%d",
               ((mech)->rd.specials2) & COMPACT_HS_TECH ? "Compact_HeatSink"
               : ((mech)->rd.specials) & (DOUBLE_HEAT_TECH | CLAN_TECH)
                   ? "Double_HeatSink"
                   : "HeatSink",
               partlist_count[put_loop]);

      /* If we are not at the end, then put a | as a spacer */
      if (put_loop < (part_count - 1)) {
        strncat(partlistbuff, "|", sizeof(buffer) - strlen(buffer) - 1);
      }

      /* Adding it to the main buffer */
      strncat(buffer, partlistbuff, sizeof(buffer) - strlen(buffer) - 1);
      break;
    default:
      snprintf(
          partlistbuff, sizeof(partlistbuff), "%s:%d",
          partname_func(mech->xcode.context, partlist_items[put_loop], 'V'),
          partlist_count[put_loop]);

      /* If we are not at the end, then put a | as a spacer */
      if (put_loop < (part_count - 1)) {
        strncat(partlistbuff, "|", sizeof(buffer) - strlen(buffer) - 1);
      }

      /* Adding it to the main buffer */
      strncat(buffer, partlistbuff, sizeof(buffer) - strlen(buffer) - 1);

      break;
    }

  } /* end printing loop */
  if (act_count) {
    snprintf(partlistbuff, sizeof(partlistbuff), "|Actuator:%d", act_count);
    strncat(buffer, partlistbuff, sizeof(buffer) - strlen(buffer) - 1);
  }

  return buffer;
}
