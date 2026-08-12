#include "mech_status_api.h"
#include "mech_status_templates_internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech_text_builder.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_parts.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"

ArmorEvaluation armor_evaluate(const ArmorEvaluationRequest *request) {
  Mech *mech = request->mech;
  const int LOC = request->section;
  const int FLAG = request->flags;
  int armor_value;
  int armor_percent, armor_denom;
  int repair_flag = 0;

  /*
   * TODO: What happens when a custom template plugs in bogus values?
   * Make sure to check for aeros, too!
   */
  switch (FLAG & ARMOR_TYPE_MASK) {
  case ARMOR_FRONT:
    /* Front armor.  */
    armor_value = mech_section_armor(mech, LOC);
    armor_denom = mech_section_original_armor(mech, LOC);

    if (mech_section_armor_repairing(mech, LOC))
      repair_flag = 1;
    break;

  case ARMOR_INTERNAL:
    if (mech_class(mech) == CLASS_AERO && LOC == 0) {
      /* Aero SI.  loc doesn't actually matter, but we check
       * it in case we want to use other locs later. */
      armor_value = mech_structural_integrity(mech);
      armor_denom = mech_original_structural_integrity(mech);
    } else {
      /* Internal armor.  */
      armor_value = mech_section_internal(mech, LOC);
      armor_denom = mech_section_original_internal(mech, LOC);

      if (mech_section_internals_repairing(mech, LOC))
        repair_flag = 1;
    }
    break;

  case ARMOR_REAR:
    /* Rear armor.  */
    armor_value = mech_section_rear_armor(mech, LOC);
    armor_denom = mech_section_original_rear_armor(mech, LOC);

    if (mech_section_rear_armor_repairing(mech, LOC))
      repair_flag = 1;
    break;

  default:
    /* FIXME: We were given a bad flag.  Panic! */
    armor_value = -1; /* XXX: hack to make problems obvious */
    repair_flag = 1;  /* XXX: hack selecting ? (repairing) */
    break;
  }

  if (repair_flag) {
    /* Under repair.  */
    return (ArmorEvaluation){.level = ARMOR_LEVEL_REPAIRING,
                             .value = armor_value};
  }
  if (!armor_value) {
    /* Breached.  */
    return (ArmorEvaluation){.level = ARMOR_LEVEL_OPEN, .value = armor_value};
  } /* Armor condition level.  */
  armor_percent = (armor_value + 1) * 100 / (armor_denom + 1);

  if (armor_percent <= 45) {
    return (ArmorEvaluation){.level = ARMOR_LEVEL_CRITICAL,
                             .value = armor_value};
  }
  if (armor_percent <= 70) {
    return (ArmorEvaluation){.level = ARMOR_LEVEL_LOW, .value = armor_value};
  }
  if (armor_percent <= 90) {
    return (ArmorEvaluation){.level = ARMOR_LEVEL_GOOD, .value = armor_value};
  }
  return (ArmorEvaluation){.level = ARMOR_LEVEL_GREAT, .value = armor_value};
}

/* bright green, dark green, bright yellow, dark red, black */
static const char *const ARMORDAMCOLORSTR[] = {
    "[fg=green bold]", "[fg=green]",      "[fg=yellow bold]",
    "[fg=red]",        "[fg=black bold]", "[fg=blue bold]"};

/* Armor location character (enemy scan). Last one is for armor under repair. */
static const char ARMORDAMLTRSTR[] = "OoxX*?";

static const char *armor_damage_color(int level) {
  if (level < 0)
    abort();
  return *(const char *const *)checked_storage_at_const(
      (const void *)ARMORDAMCOLORSTR,
      sizeof(ARMORDAMCOLORSTR) / sizeof(*ARMORDAMCOLORSTR),
      sizeof(*ARMORDAMCOLORSTR), (size_t)level);
}

static char armor_damage_letter(int level) {
  if (level < 0)
    abort();
  return *checked_string_suffix(ARMORDAMLTRSTR, (size_t)level);
}

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

typedef struct ArmorDamageTextRequest {
  int level;
  int value;
  int flags;
  size_t width;
} ArmorDamageTextRequest;

static ArmorDamageText
armor_damage_text(const ArmorDamageTextRequest *request) {
  ArmorDamageText result = {0};
  char armor_buf[23 + 1];
  int armor_value = request->value;

  if (request->flags & ARMOR_FLAG_DIVIDE_10) {
    /* Divide by 10 (rounded up).  Used for mechwarriors.  */
    armor_value = (armor_value + 9) / 10;
  }

  if (request->flags & ARMOR_FLAG_OWNED) {
    size_t armor_len;

    /* TODO: snprintf() is a C99-ism, please autoconf-ize.  */
    /* XXX: Aeros 0-filled spaces.  That's silly.  */
    (void)snprintf(armor_buf, sizeof(armor_buf), "%d", armor_value);

    /* XXX: Return values aren't standardized until C99.  */
    armor_len = strlen(armor_buf);

    /* Fixed width.  Some snprintf()s have a $*d extension that we
     * aren't going to use.  */
    if (armor_len < request->width) {
      /* Right justify.  */
      const size_t PADDING = request->width - armor_len;
      memset(
          checked_storage_region(result.text, sizeof(result.text), 0, PADDING),
          ' ', PADDING);
      memcpy(checked_storage_region(result.text, sizeof(result.text), PADDING,
                                    armor_len),
             armor_buf, armor_len);
    } else {
      /* Right truncate.  */
      memcpy(checked_storage_region(result.text, sizeof(result.text), 0,
                                    request->width),
             checked_string_suffix(armor_buf, armor_len - request->width),
             request->width);
    }
  } else {
    /* Use adversarial (scan) fill characters.  */
    memset(result.text, armor_damage_letter(request->level), request->width);
  }

  *(char *)checked_storage_at(result.text, sizeof(result.text), sizeof(char),
                              request->width) = '\0';

  return result;
}

/*
 * TODO: Probably better to make this a substitution.  That would allow the key
 * width to match the actual width on the status display, too; right now, it's
 * always two characters, regardless of width.
 */
static ArmorKeyText armor_key_text(int line_key, bool owner) {
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
    const int ARMOR_LEVEL = 6 - line_key;
    const char LETTER = armor_damage_letter(ARMOR_LEVEL);
    (void)snprintf(result.text, sizeof(result.text), "%s%c%c [reset]",
                   armor_damage_color(ARMOR_LEVEL), LETTER, LETTER);
  }

  return result;
}

/*
 * XXX: memcpy/memset() are technically only standard as of C99, so strictly we
 * should autoconf-ize this with portability wrappers.  They're pretty common
 * these days, though.
 */
static ArmorFieldText armor_field_text(Mech *mech, const int LOC,
                                       const int FLAG, int width) {
  ArmorFieldText result = {0};

  int armor_level, armor_value;

  /* Sanity check arguments.  */
  if (width < 0)
    width = 0;
  else if (width > 23)
    width = 23;

  /* Get armor status.  */
  const ArmorEvaluation EVALUATION = armor_evaluate(&(ArmorEvaluationRequest){
      .mech = mech,
      .section = LOC,
      .flags = FLAG,
  });
  armor_level = EVALUATION.level;
  armor_value = EVALUATION.value;

  /* Get strings.  */
  if (!(FLAG & ARMOR_FLAG_SHOW_DEST) && !mech_section_internal(mech, LOC)) {
    /* Blank field. (Destroyed section.) */
    memset(result.text, ' ', (size_t)width);
    *(char *)checked_storage_at(result.text, sizeof(result.text), sizeof(char),
                                (size_t)width) = '\0';
    return result;
  }

  ArmorDamageText damage =
      armor_damage_text(&(ArmorDamageTextRequest){.level = armor_level,
                                                  .value = armor_value,
                                                  .flags = FLAG,
                                                  .width = (size_t)width});
  (void)snprintf(result.text, sizeof(result.text), "%s%s[reset]",
                 armor_damage_color(armor_level), damage.text);

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
  string_copy(result, status.rendered);
  return true;
}

/* BTS = BattleTech status. */
typedef enum {
  BTS_START_OF_LINE, /* start state */

  BTS_NORMAL, /* normal input */

  BTS_SUBSTITUTE_ARMOR, /* armor status substitution */
  BTS_CONDITIONAL_1,    /* unary conditional  */
  BTS_CONDITIONAL_2     /* binary conditional */
} BtsState;

static void armor_template_commit(BtechTextBuilder *destination,
                                  const char *source, size_t *saved_position,
                                  size_t source_position) {
  if (source_position < *saved_position)
    abort();
  const size_t LENGTH = source_position - *saved_position;
  btech_text_builder_append_count(
      destination, checked_string_suffix(source, *saved_position), LENGTH);
  *saved_position = source_position;
}

static bool ascii_is_digit(char value) { return value >= '0' && value <= '9'; }

static int ascii_digit_value(int value) { return value - '0'; }

void print_armor_status(EvaluationContext *evaluation, DbRef player, Mech *mech,
                        int owner) {
  const char *srcbuf;
  char destbuf[LBUF_SIZE];

  BtsState current_state = BTS_START_OF_LINE;
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
      srcbuf = MWDESC;
      break;

    case CLASS_MECH:
      if (mech_movement_type(mech) == MOVE_QUAD) {
        srcbuf = QUADDESC;
      } else {
#ifdef WEIGHTVARIABLE_STATUS
        if (mech_tonnage(mech) <= 35)
          srcbuf = LIGHTMECHDESC;
        else if (mech_tonnage(mech) <= 55)
          srcbuf = MEDIUMMECHDESC;
        else if (mech_tonnage(mech) <= 75)
          srcbuf = HEAVYMECHDESC;
        else
          srcbuf = ASSAULTMECHDESC;
#else  /* WEIGHTVARIABLE_STATUS */
        srcbuf = mechdesc;
#endif /* WEIGHTVARIABLE_STATUS */
      }
      break;

    case CLASS_BSUIT:
      srcbuf = BSUITDESC;
      break;

    case CLASS_VTOL:
      srcbuf = VTOLDESC;
      break;

    case CLASS_AERO:
      srcbuf = AERODESC;
      break;

    case CLASS_DS:
      srcbuf = AEROD_DS_DESC;
      break;

    case CLASS_SPHEROID_DS:
      srcbuf = SPHER_DS_DESC;
      break;

    case CLASS_VEH_GROUND:
      if (mech_section_original_internal(mech, TURRET))
        srcbuf = VEHDESC;
      else
        srcbuf = VEH_NOT_DESC;
      break;

    case CLASS_VEH_NAVAL:
      if (mech_movement_type(mech) == MOVE_FOIL)
        srcbuf = FOILDESC;
      else if (mech_movement_type(mech) == MOVE_HULL)
        srcbuf = SHIPDESC;
      else
        srcbuf = SUBDESC;
      break;

    default:
      srcbuf = " This 'toy' is of unknown type. It has yet to be templated\n "
               "for status.";
      break;
    }
  }

  /* Perform substitution on template.  */
  BtechTextBuilder destination;
  btech_text_builder_initialize(&destination, destbuf, sizeof(destbuf));
  size_t saved_source_position = 0;
  const size_t SOURCE_LENGTH = strlen(srcbuf);
  for (size_t source_position = 0; source_position < SOURCE_LENGTH;
       source_position++) {
    const char SOURCE_CHARACTER =
        *checked_string_suffix(srcbuf, source_position);
    BtsState next_state = current_state;

    /* Dispatch on current state.  */
    switch (current_state) {
    case BTS_START_OF_LINE: /* start of line */
      /*
       * XXX: Portability note: Depends on a specific way of
       * encoding the digits from 0 to 7.
       */
      if (SOURCE_CHARACTER >= '1' && SOURCE_CHARACTER <= '7') {
        armor_template_commit(&destination, srcbuf, &saved_source_position,
                              source_position);
        saved_source_position = source_position + 1;

        ArmorKeyText key =
            armor_key_text(ascii_digit_value(SOURCE_CHARACTER), owner);
        btech_text_builder_append(&destination, key.text);
      }

      next_state = BTS_NORMAL;
      break;

    case BTS_NORMAL: /* normal characters */
      switch (SOURCE_CHARACTER) {
      case '&':
        armor_template_commit(&destination, srcbuf, &saved_source_position,
                              source_position);
        next_state = BTS_SUBSTITUTE_ARMOR;
        break;

      case '@':
        armor_template_commit(&destination, srcbuf, &saved_source_position,
                              source_position);
        next_state = BTS_CONDITIONAL_1;
        break;

      case '!':
        armor_template_commit(&destination, srcbuf, &saved_source_position,
                              source_position);
        next_state = BTS_CONDITIONAL_2;
        break;
      }
      break;

    case BTS_SUBSTITUTE_ARMOR: /* armor status substitution */
      switch (source_position - saved_source_position) {
        int tmp_flag;

      case 1: /* optional width digit or type flag */
        switch (SOURCE_CHARACTER) {
        case '&':
          saved_source_position = source_position + 1;
          next_state = BTS_NORMAL;

          btech_text_builder_append_character(&destination, '&');
          break;

        case '+':
        case ':':
        case '-':
          tmp_value1 = (unsigned char)SOURCE_CHARACTER;
          break;

        default:
          if (ascii_is_digit(SOURCE_CHARACTER)) {
            tmp_value1 = (unsigned char)SOURCE_CHARACTER;
          } else {
            next_state = BTS_NORMAL;
          }
          break;
        }
        break;

      case 2: /* location or type flag */
        if (ascii_is_digit((char)tmp_value1)) {
          /* Expect type code.  */
          switch (SOURCE_CHARACTER) {
          case '+':
          case '-':
          case ':':
            tmp_value2 = (unsigned char)SOURCE_CHARACTER;
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
        ArmorFieldText field = armor_field_text(
            mech, ascii_digit_value(SOURCE_CHARACTER), tmp_flag, tmp_value1);
        btech_text_builder_append(&destination, field.text);

        saved_source_position = source_position + 1;
        next_state = BTS_NORMAL;
        break;

      default: /* XXX: should never happen */
        break;
      }
      break;

    case BTS_CONDITIONAL_1: /* '@' unary conditional */
      switch (source_position - saved_source_position) {
      case 1: /* get critical section */
        if (ascii_is_digit(SOURCE_CHARACTER)) {
          tmp_value1 = ascii_digit_value(SOURCE_CHARACTER);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 2: /* copy conditional character */
        saved_source_position = source_position + 1;
        next_state = BTS_NORMAL;

        if (mech_section_internal(mech, tmp_value1)) {
          btech_text_builder_append_character(&destination, SOURCE_CHARACTER);
        } else {
          btech_text_builder_append_character(&destination, ' ');
        }
        break;

      default: /* XXX: should never happen */
        break;
      }
      break;

    case BTS_CONDITIONAL_2: /* '!' binary conditional */
      switch (source_position - saved_source_position) {
      case 1: /* get first critical section */
        if (ascii_is_digit(SOURCE_CHARACTER)) {
          tmp_value1 = ascii_digit_value(SOURCE_CHARACTER);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 2: /* get second critical section */
        if (ascii_is_digit(SOURCE_CHARACTER)) {
          tmp_value2 = ascii_digit_value(SOURCE_CHARACTER);
        } else {
          next_state = BTS_NORMAL;
        }
        break;

      case 3: /* copy conditional character */
        saved_source_position = source_position + 1;
        next_state = BTS_NORMAL;

        if (mech_section_internal(mech, tmp_value1) ||
            mech_section_internal(mech, tmp_value2)) {
          btech_text_builder_append_character(&destination, SOURCE_CHARACTER);
        } else {
          btech_text_builder_append_character(&destination, ' ');
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
    if (SOURCE_CHARACTER == '\n') {
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
      armor_template_commit(&destination, srcbuf, &saved_source_position,
                            source_position);
      btech_text_builder_append_character(&destination, '\r');
      /* \n written later.  */
    } else {
      current_state = next_state;
    }
  }

  /* Finish up.  */
  armor_template_commit(&destination, srcbuf, &saved_source_position,
                        SOURCE_LENGTH);

  /* Send formatted status.  */
  mecha_notify(evaluation, player, destbuf);
}

/*
 * Figure out if we have a certain kind of physical weapon.
 */
bool has_physical(const PhysicalWeaponRequest *request) {
  Mech *obj_mech = request->mech;
  const int W_LOC = request->section;
  const MechPhysicalWeaponType W_PHYS_TYPE = request->type;
  int w_type;
  int w_size;

  switch (W_PHYS_TYPE) {
  case PHY_AXE:
    w_type = AXE;
    w_size = mech_tonnage(obj_mech) / 15;
    break;

  case PHY_CLAW:
    w_type = CLAW;
    w_size = mech_tonnage(obj_mech) / 15;
    break;

  case PHY_SWORD:
    w_type = SWORD;
    w_size = mech_tonnage(obj_mech) / 15;
    break;

  case PHY_MACE:
    w_type = MACE;
    w_size = mech_tonnage(obj_mech) / 15;
    break;

  case PHY_SAW:
    w_type = DUAL_SAW;
    w_size = 7;
    break;

  default:
    return false;
  } // end switch()

  return find_obj_with_dest(obj_mech, W_LOC, special_equipment_index(w_type)) >=
         w_size;
} // end hasPhysical()

bool can_use_physical(const PhysicalWeaponRequest *request) {
  Mech *obj_mech = request->mech;
  const int W_LOC = request->section;
  const MechPhysicalWeaponType W_PHYS_TYPE = request->type;
  bool t_ret = true;

  switch (W_PHYS_TYPE) {
  case PHY_AXE:
  case PHY_SWORD:
    if (mech_section_is_destroyed(obj_mech, W_LOC))
      t_ret = false;
    else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = obj_mech,
                 .slot = {.section = W_LOC, .critical = 0},
                 .special = SHOULDER_OR_HIP}))
      t_ret = false;
    else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = obj_mech,
                 .slot = {.section = W_LOC, .critical = 3},
                 .special = HAND_OR_FOOT_ACTUATOR}))
      t_ret = false;
    break;

  case PHY_CLAW:
    if (mech_section_is_destroyed(obj_mech, W_LOC))
      t_ret = false;
    break;

  case PHY_MACE:
    if (mech_section_is_destroyed(obj_mech, W_LOC))
      t_ret = false;
    else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = obj_mech,
                 .slot = {.section = W_LOC, .critical = 0},
                 .special = SHOULDER_OR_HIP}))
      t_ret = false;
    else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = obj_mech,
                 .slot = {.section = W_LOC, .critical = 3},
                 .special = HAND_OR_FOOT_ACTUATOR}))
      t_ret = false;
    break;

  case PHY_SAW:
    if (mech_section_is_destroyed(obj_mech, W_LOC))
      t_ret = false;
    else if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                 .mech = obj_mech,
                 .slot = {.section = W_LOC, .critical = 0},
                 .special = SHOULDER_OR_HIP}))
      t_ret = false;
    break;

  default:
    t_ret = false;
  } // end switch()

  return t_ret;
} // end canUsePhysical()
