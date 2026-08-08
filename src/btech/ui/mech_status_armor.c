#include "mech_status_api.h"
#include "mech_status_templates_internal.h"

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

ArmorLevel ArmorEvaluateSerious(Mech *mech, int loc, int flag,
                                int *ret_armor_value) {
  int armor_value;
  int armor_percent, armor_denom;
  int repair_flag = 0;

  /*
   * TODO: What happens when a custom template plugs in bogus values?
   * Make sure to check for aeros, too!
   */
  switch (flag & ARMOR_TYPE_MASK) {
  case ARMOR_FRONT:
    /* Front armor.  */
    armor_value = mech_section_armor(mech, loc);
    armor_denom = mech_section_original_armor(mech, loc);

    if (mech_section_armor_repairing(mech, loc))
      repair_flag = 1;
    break;

  case ARMOR_INTERNAL:
    if (mech_class(mech) == CLASS_AERO && loc == 0) {
      /* Aero SI.  loc doesn't actually matter, but we check
       * it in case we want to use other locs later. */
      armor_value = mech_structural_integrity(mech);
      armor_denom = mech_original_structural_integrity(mech);
    } else {
      /* Internal armor.  */
      armor_value = mech_section_internal(mech, loc);
      armor_denom = mech_section_original_internal(mech, loc);

      if (mech_section_internals_repairing(mech, loc))
        repair_flag = 1;
    }
    break;

  case ARMOR_REAR:
    /* Rear armor.  */
    armor_value = mech_section_rear_armor(mech, loc);
    armor_denom = mech_section_original_rear_armor(mech, loc);

    if (mech_section_rear_armor_repairing(mech, loc))
      repair_flag = 1;
    break;

  default:
    /* FIXME: We were given a bad flag.  Panic! */
    armor_value = -1; /* XXX: hack to make problems obvious */
    repair_flag = 1;  /* XXX: hack selecting ? (repairing) */
    break;
  }

  if (ret_armor_value) {
    *ret_armor_value = armor_value;
  }

  if (repair_flag) {
    /* Under repair.  */
    return ARMOR_LEVEL_REPAIRING;
  } else if (!armor_value) {
    /* Breached.  */
    return ARMOR_LEVEL_OPEN;
  } else {
    /* Armor condition level.  */
    armor_percent = (armor_value + 1) * 100 / (armor_denom + 1);

    if (armor_percent <= 45) {
      return ARMOR_LEVEL_CRITICAL;
    } else if (armor_percent <= 70) {
      return ARMOR_LEVEL_LOW;
    } else if (armor_percent <= 90) {
      return ARMOR_LEVEL_GOOD;
    } else {
      return ARMOR_LEVEL_GREAT;
    }
  }
}

/* bright green, dark green, bright yellow, dark red, black */
static const char *const armordamcolorstr[] = {
    "[fg=green bold]", "[fg=green]",      "[fg=yellow bold]",
    "[fg=red]",        "[fg=black bold]", "[fg=blue bold]"};

/* Armor location character (enemy scan). Last one is for armor under repair. */
static const char armordamltrstr[] = "OoxX*?";

/*
 * XXX: memcpy/memset() are technically only standard as of C99, so strictly we
 * should autoconf-ize this with portability wrappers.  They're pretty common
 * these days, though.
 */
typedef struct ArmorDamageText {
  char text[23 + 1];
} ArmorDamageText;

typedef struct ArmorKeyText {
  char text[64];
} ArmorKeyText;

typedef struct ArmorFieldText {
  char text[64];
} ArmorFieldText;

static ArmorDamageText armor_damage_text(const int armor_level, int armor_value,
                                         const int flag, const size_t width) {
  ArmorDamageText result = {0};
  char *asp;

  char armor_buf[23 + 1];

  if (flag & ARMOR_FLAG_DIVIDE_10) {
    /* Divide by 10 (rounded up).  Used for mechwarriors.  */
    armor_value = (armor_value + 9) / 10;
  }

  if (flag & ARMOR_FLAG_OWNED) {
    size_t armor_len;

    /* TODO: snprintf() is a C99-ism, please autoconf-ize.  */
    /* XXX: Aeros 0-filled spaces.  That's silly.  */
    snprintf(armor_buf, sizeof(armor_buf), "%d", armor_value);

    /* XXX: Return values aren't standardized until C99.  */
    armor_len = strlen(armor_buf);

    /* Fixed width.  Some snprintf()s have a $*d extension that we
     * aren't going to use.  */
    asp = result.text;

    if (armor_len < width) {
      /* Right justify.  */
      memset(asp, ' ', width - armor_len);
      asp += width - armor_len;

      memcpy(asp, armor_buf, armor_len);
      asp += armor_len;
    } else {
      /* Right truncate.  */
      memcpy(asp, armor_buf + (armor_len - width), width);
      asp += width;
    }
  } else {
    /* Use adversarial (scan) fill characters.  */
    memset(result.text, armordamltrstr[(size_t)armor_level], width);
  }

  result.text[width] = '\0';

  return result;
}

/*
 * TODO: Probably better to make this a substitution.  That would allow the key
 * width to match the actual width on the status display, too; right now, it's
 * always two characters, regardless of width.
 */
static ArmorKeyText armor_key_text(int line_key, int owner) {
  ArmorKeyText result = {0};

  if (owner) {
    /* Only show key on scans.  */
    result.text[0] = '\0';
  } else if (line_key == 1) {
    /* Line 1 = "Key".  */
    strcpy(result.text, "Key");
  } else if (line_key > 6) {
    /* Line >6 = empty.  */
    strcpy(result.text, "   ");
  } else {
    /* Line 2-6 = armor level symbols.  */
    /* XXX: Probably safe from buffer overflows.  */
    snprintf(result.text, sizeof(result.text), "%s%c%c [reset]",
             armordamcolorstr[6 - line_key], armordamltrstr[6 - line_key],
             armordamltrstr[6 - line_key]);
  }

  return result;
}

/*
 * XXX: memcpy/memset() are technically only standard as of C99, so strictly we
 * should autoconf-ize this with portability wrappers.  They're pretty common
 * these days, though.
 */
static ArmorFieldText armor_field_text(Mech *mech, const int loc,
                                       const int flag, int width) {
  ArmorFieldText result = {0};

  int armor_level, armor_value;

  /* Sanity check arguments.  */
  if (width < 0)
    width = 0;
  else if (width > 23)
    width = 23;

  /* Get armor status.  */
  armor_level = ArmorEvaluateSerious(mech, loc, flag, &armor_value);

  /* Get strings.  */
  if (!(flag & ARMOR_FLAG_SHOW_DEST) && !mech_section_internal(mech, loc)) {
    /* Blank field. (Destroyed section.) */
    memset(result.text, ' ', (size_t)width);
    result.text[width] = '\0';
    return result;
  }

  ArmorDamageText damage =
      armor_damage_text(armor_level, armor_value, flag, (size_t)width);
  snprintf(result.text, sizeof(result.text), "%s%s[reset]",
           armordamcolorstr[(size_t)armor_level], damage.text);

  return result;
}

static bool get_lua_status_template(EvaluationContext *evaluation, DbRef player,
                                    Mech *mech, char *result) {
  LuaMechStatusResult status;
  lua_mech_status_evaluate(
      evaluation->runtime->lua_owner->runtime,
      &(LuaMechStatusInvocation){
          .descriptor =
              evaluation->command ? evaluation->command->descriptor : nullptr,
          .object = mech_dbref(mech),
          .enactor = player,
          .cause = mech_dbref(mech),
      },
      &status);
  if (!status.defined)
    return false;
  StringCopy(result, status.rendered);
  return true;
}

/* BTS = BattleTech status. */
typedef enum {
  BTS_START_OF_LINE, /* start state */

  BTS_NORMAL, /* normal input */

  BTS_SUBSTITUTE_ARMOR, /* armor status substitution */
  BTS_CONDITIONAL_1,    /* unary conditional  */
  BTS_CONDITIONAL_2     /* binary conditional */
} BTS_State;

static void armor_template_commit(char destination[static LBUF_SIZE],
                                  char **destination_position,
                                  const char **saved_source,
                                  const char *source_position) {
  size_t destination_length = (size_t)(*destination_position - destination);
  size_t source_length = (size_t)(source_position - *saved_source);
  size_t available = LBUF_SIZE - 1 - destination_length;
  if (source_length > available)
    source_length = available;
  memcpy(*destination_position, *saved_source, source_length);
  *destination_position += source_length;
  *saved_source = source_position;
}

static void armor_template_append(char destination[static LBUF_SIZE],
                                  char **position, char value) {
  if ((size_t)(*position - destination) < LBUF_SIZE - 1)
    *(*position)++ = value;
}

static int ascii_digit_value(int value) { return value - '0'; }

void PrintArmorStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                      int owner) {
  const char *srcbuf, *sbp, *saved_sbp;

  char destbuf[LBUF_SIZE], *dbp;

  BTS_State current_state = BTS_START_OF_LINE;
  int tmp_value1 = 0, tmp_value2 = 0;
  int flag;

  char tmpbuf[8192];

  /* Select status template.  */
  switch (mech_class(mech)) {
  case CLASS_MECH:
  case CLASS_VEH_GROUND:
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_BSUIT:
    flag = 0;
    break;

  case CLASS_MW:
    /* TODO: Should probably make this user-selectable by adding
     * some more formatting flags.  */
    flag = ARMOR_FLAG_DIVIDE_10;
    break;

  case CLASS_AERO:
  case CLASS_DS:
    flag = ARMOR_FLAG_SHOW_DEST;
    break;

  default:
    flag = 0;
    break;
  }

  if (get_lua_status_template(evaluation, player, mech, tmpbuf)) {
    /* Use custom template.  */
    srcbuf = tmpbuf;
  } else {
    /* Use standard template.  */
    switch (mech_class(mech)) {
    case CLASS_MW:
      srcbuf = mwdesc;
      break;

    case CLASS_MECH:
      if (mech_movement_type(mech) == MOVE_QUAD) {
        srcbuf = quaddesc;
      } else {
#ifdef WEIGHTVARIABLE_STATUS
        if (mech_tonnage(mech) <= 35)
          srcbuf = lightmechdesc;
        else if (mech_tonnage(mech) <= 55)
          srcbuf = mediummechdesc;
        else if (mech_tonnage(mech) <= 75)
          srcbuf = heavymechdesc;
        else
          srcbuf = assaultmechdesc;
#else  /* WEIGHTVARIABLE_STATUS */
        srcbuf = mechdesc;
#endif /* WEIGHTVARIABLE_STATUS */
      }
      break;

    case CLASS_BSUIT:
      srcbuf = bsuitdesc;
      break;

    case CLASS_VTOL:
      srcbuf = vtoldesc;
      break;

    case CLASS_AERO:
      srcbuf = aerodesc;
      break;

    case CLASS_DS:
      srcbuf = aerod_ds_desc;
      break;

    case CLASS_SPHEROID_DS:
      srcbuf = spher_ds_desc;
      break;

    case CLASS_VEH_GROUND:
      if (mech_section_original_internal(mech, TURRET))
        srcbuf = vehdesc;
      else
        srcbuf = veh_not_desc;
      break;

    case CLASS_VEH_NAVAL:
      if (mech_movement_type(mech) == MOVE_FOIL)
        srcbuf = foildesc;
      else if (mech_movement_type(mech) == MOVE_HULL)
        srcbuf = shipdesc;
      else
        srcbuf = subdesc;
      break;

    default:
      srcbuf = " This 'toy' is of unknown type. It has yet to be templated\n "
               "for status.";
      break;
    }
  }

  /* Perform substitution on template.  */
  dbp = destbuf;

  saved_sbp = srcbuf;
  for (sbp = srcbuf; *sbp; sbp++) {
    BTS_State next_state = current_state;

    /* Dispatch on current state.  */
    switch (current_state) {
    case BTS_START_OF_LINE: /* start of line */
      /*
       * XXX: Portability note: Depends on a specific way of
       * encoding the digits from 0 to 7.
       */
      if (*sbp >= '1' && *sbp <= '7') {
        armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);
        saved_sbp = sbp + 1;

        safe_str(armor_key_text(ascii_digit_value(*sbp), owner).text, destbuf,
                 &dbp);
      }

      next_state = BTS_NORMAL;
      break;

    case BTS_NORMAL: /* normal characters */
      switch (*sbp) {
      case '&':
        armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);
        next_state = BTS_SUBSTITUTE_ARMOR;
        break;

      case '@':
        armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);
        next_state = BTS_CONDITIONAL_1;
        break;

      case '!':
        armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);
        next_state = BTS_CONDITIONAL_2;
        break;
      }
      break;

    case BTS_SUBSTITUTE_ARMOR: /* armor status substitution */
      switch (sbp - saved_sbp) {
        int tmp_flag;

      case 1: /* optional width digit or type flag */
        switch (*sbp) {
        case '&':
          saved_sbp = sbp + 1;
          next_state = BTS_NORMAL;

          armor_template_append(destbuf, &dbp, '&');
          break;

        case '+':
        case ':':
        case '-':
          tmp_value1 = *sbp;
          break;

        default:
          if (isdigit(*sbp)) {
            tmp_value1 = *sbp;
          } else {
            next_state = BTS_NORMAL;
          }
          break;
        }
        break;

      case 2: /* location or type flag */
        if (isdigit(tmp_value1)) {
          /* Expect type code.  */
          switch (*sbp) {
          case '+':
          case '-':
          case ':':
            tmp_value2 = *sbp;
            break;

          default:
            next_state = BTS_NORMAL;
            break;
          }

          tmp_value1 = ascii_digit_value(tmp_value1);
          break;
        } else {
          /* Expect section number.  */
          tmp_value2 = tmp_value1;
          tmp_value1 = 2;
          [[fallthrough]];
        }
      case 3: /* location */
        /* Expect section number.  */
        tmp_flag = flag;

        tmp_flag |= owner ? ARMOR_FLAG_OWNED : 0;

        switch (tmp_value2) {
        case '+':
          tmp_flag |= ARMOR_FRONT;
          break;

        case ':':
          tmp_flag |= ARMOR_INTERNAL;
          break;

        case '-':
          tmp_flag |= ARMOR_REAR;
          break;

        default:
          break;
        }

        /* FIXME: Ponder semantics of gflag.  */
        safe_str(armor_field_text(mech, ascii_digit_value(*sbp), tmp_flag,
                                  tmp_value1)
                     .text,
                 destbuf, &dbp);

        saved_sbp = sbp + 1;
        next_state = BTS_NORMAL;
        break;

      default: /* XXX: should never happen */
        break;
      }
      break;

    case BTS_CONDITIONAL_1: /* '@' unary conditional */
      switch (sbp - saved_sbp) {
      case 1: /* get critical section */
        if (isdigit(*sbp)) {
          tmp_value1 = ascii_digit_value(*sbp);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 2: /* copy conditional character */
        saved_sbp = sbp + 1;
        next_state = BTS_NORMAL;

        if (mech_section_internal(mech, tmp_value1)) {
          armor_template_append(destbuf, &dbp, *sbp);
        } else {
          armor_template_append(destbuf, &dbp, ' ');
        }
        break;

      default: /* XXX: should never happen */
        break;
      }
      break;

    case BTS_CONDITIONAL_2: /* '!' binary conditional */
      switch (sbp - saved_sbp) {
      case 1: /* get first critical section */
        if (isdigit(*sbp)) {
          tmp_value1 = ascii_digit_value(*sbp);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 2: /* get second critical section */
        if (isdigit(*sbp)) {
          tmp_value2 = ascii_digit_value(*sbp);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 3: /* copy conditional character */
        saved_sbp = sbp + 1;
        next_state = BTS_NORMAL;

        if (mech_section_internal(mech, tmp_value1) ||
            mech_section_internal(mech, tmp_value2)) {
          armor_template_append(destbuf, &dbp, *sbp);
        } else {
          armor_template_append(destbuf, &dbp, ' ');
        }
        break;

      default: /* XXX: should never happen */
        break;
      }
      break;

    default: /* XXX: should never happen */
      break;
    }

    /* Common logic.  */
    if (*sbp == '\n') {
      current_state = BTS_START_OF_LINE;

      /*
       * MUX expects \r\n for line endings in buffers.
       * PennMUSH, where this code was originally developed,
       * expects \n, and converts to \r\n as needed.
       *
       * FIXME: This is sorta a hack.  We don't really want
       * to be dealing with line ending issues in individual
       * functions, but more extensive changes would be
       * disruptive.
       */
      armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);
      armor_template_append(destbuf, &dbp, '\r');
      /* \n written later.  */
    } else {
      current_state = next_state;
    }
  }

  /* Finish up.  */
  armor_template_commit(destbuf, &dbp, &saved_sbp, sbp);

  /* Send formatted status.  */
  *dbp = '\0';

  mecha_notify(evaluation, player, destbuf);
}

/*
 * Figure out if we have a certain kind of physical weapon.
 */
int hasPhysical(Mech *objMech, int wLoc, int wPhysType) {
  int wType;
  int wSize;

  switch (wPhysType) {
  case PHY_AXE:
    wType = AXE;
    wSize = mech_tonnage(objMech) / 15;
    break;

  case PHY_CLAW:
    wType = CLAW;
    wSize = mech_tonnage(objMech) / 15;
    break;

  case PHY_SWORD:
    wType = SWORD;
    wSize = mech_tonnage(objMech) / 15;
    break;

  case PHY_MACE:
    wType = MACE;
    wSize = mech_tonnage(objMech) / 15;
    break;

  case PHY_SAW:
    wType = DUAL_SAW;
    wSize = 7;
    break;

  default:
    return 0;
  } // end switch()

  return FindObjWithDest(objMech, wLoc, special_equipment_index(wType)) >=
         wSize;
} // end hasPhysical()

int canUsePhysical(Mech *objMech, int wLoc, int wPhysType) {
  int tRet = 1;

  switch (wPhysType) {
  case PHY_AXE:
  case PHY_SWORD:
    if (mech_section_is_destroyed(objMech, wLoc))
      tRet = 0;
    else if (!mech_critical_is_operational_special(objMech, wLoc, 0,
                                                   SHOULDER_OR_HIP))
      tRet = 0;
    else if (!mech_critical_is_operational_special(objMech, wLoc, 3,
                                                   HAND_OR_FOOT_ACTUATOR))
      tRet = 0;
    break;

  case PHY_CLAW:
    if (mech_section_is_destroyed(objMech, wLoc))
      tRet = 0;
    break;

  case PHY_MACE:
    if (mech_section_is_destroyed(objMech, wLoc))
      tRet = 0;
    else if (!mech_critical_is_operational_special(objMech, wLoc, 0,
                                                   SHOULDER_OR_HIP))
      tRet = 0;
    else if (!mech_critical_is_operational_special(objMech, wLoc, 3,
                                                   HAND_OR_FOOT_ACTUATOR))
      tRet = 0;
    break;

  case PHY_SAW:
    if (mech_section_is_destroyed(objMech, wLoc))
      tRet = 0;
    else if (!mech_critical_is_operational_special(objMech, wLoc, 0,
                                                   SHOULDER_OR_HIP))
      tRet = 0;
    break;

  default:
    tRet = 0;
  } // end switch()

  return tRet;
} // end canUsePhysical()
