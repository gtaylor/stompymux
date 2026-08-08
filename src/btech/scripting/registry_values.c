#include "registry_api.h"

#include "checked_conversion.h"

// The static value catalog guarantees source-kind/type pairings. Clang's
// analyzer cannot infer that a field-only type always has a field offset.
// NOLINTBEGIN(clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference)
#include "values_internal.h"

/* INDENT OFF */
const int scode_in_out[TYPE_LAST_TYPE] =
    /* st ch sh in fl db sf sfb sfs bv sfbd cbv ro-ch ro-sh ro-in ro-fl ro-db*/
    {3, 3, 3, 3, 3, 3, 1, 1, 2, 3, 3, 3, 3, 1, 1, 1, 1, 1};
/* INDENT ON */

static bool mech_value_write_text(Mech *mech, const GMV *descriptor,
                                  const char *text) {
  MechScriptValue value = {0};

  switch (descriptor->type) {
  case TYPE_STRING:
    value.string = text;
    break;
  case TYPE_DBREF:
    value.dbref = atoi(text);
    break;
  case TYPE_FLOAT:
    value.floating = strtof(text, nullptr);
    break;
  case TYPE_BV:
  case TYPE_CBV:
    value.integer = text2bv(text);
    break;
  default:
    value.integer = atoi(text);
    break;
  }

  return mech_script_value_write(mech, descriptor->source.mech_key, value);
}

static char *mech_value_read_text(const Mech *mech, const GMV *descriptor,
                                  char *buffer) {
  MechScriptValue value = {0};
  if (!mech_script_value_read(mech, descriptor->source.mech_key, &value))
    return nullptr;

  switch (descriptor->type) {
  case TYPE_STRING:
    snprintf(buffer, LBUF_SIZE, "%s", value.string);
    break;
  case TYPE_DBREF:
  case TYPE_DBREF_RO:
    snprintf(buffer, LBUF_SIZE, "%ld", value.dbref);
    break;
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO:
    snprintf(buffer, LBUF_SIZE, "%.2f", (double)value.floating);
    break;
  case TYPE_BV:
  case TYPE_CBV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text(value.integer, (char[SBUF_SIZE]){0}));
    break;
  default:
    snprintf(buffer, LBUF_SIZE, "%d", value.integer);
    break;
  }
  return buffer;
}

void fun_zmechs(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  size_t len = 0;
  char reference[SBUF_SIZE];

  if (!is_controls(context->world->database, player, it) &&
      !is_wizard(context->btech->database, player)) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->btech->database->top; i++)
    if (typeof_obj(context->btech->database, i) == OBJECT_TYPE_THING) {
      if (game_object_zone(context->btech->database, i) == it) {
        if ((btech_context_which_special(context->btech, i) == GTYPE_MECH) &&
            is_good_obj(context->btech->database, i)) {
          if (len) {
            snprintf(reference, sizeof(reference), " #%ld", i);
            if ((strlen(reference) + len) > (LBUF_SIZE - SBUF_SIZE)) {
              safe_str(" #-1", buff, bufc);
              return;
            }
            safe_str(reference, buff, bufc);
            len += strlen(reference);
          } else {
            safe_tprintf_str(buff, bufc, "#%ld", i);
            len = strlen(buff);
          }
        }
      }
    }
}

void fun_btsetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = name of the value
     fargs[2] = what the value's to be set as
   */
  DbRef it;
  int i, spec;
  void *foo;
  void *bar;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  spec = btech_context_which_special(context->btech, it);
  if (!(foo = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 2)) {
      if (xcode_data[i].source_kind == GMV_SOURCE_MECH_KEY) {
        if (mech_value_write_text(foo, &xcode_data[i], fargs[2])) {
          safe_tprintf_str(buff, bufc, "1");
          return;
        }
        break;
      }
      bar = xcode_data[i].source_kind == GMV_SOURCE_FIELD_OFFSET
                ? (char *)foo + xcode_data[i].source.field_offset
                : nullptr;
      switch (xcode_data[i].type) {
      case TYPE_STRFUNC_BD:
      case TYPE_STRFUNC_S:
        xcode_data[i].source.bidirectional_callback(1, (Mech *)foo,
                                                    (char *)fargs[2]);
        break;
      case TYPE_STRFUNC_BD_BUF:
        xcode_data[i].source.buffered_bidirectional_callback(
            1, (Mech *)foo, (char *)fargs[2], (char[LBUF_SIZE]){0});
        break;
      case TYPE_STRING: {
        const size_t capacity = (size_t)xcode_data[i].size;
        if (capacity == 0)
          break;
        strncpy(bar, fargs[2], capacity - 1);
        ((char *)bar)[capacity - 1] = '\0';
        break;
      }
      case TYPE_DBREF:
        *((DbRef *)bar) = atoi(fargs[2]);
        break;
      case TYPE_CHAR:
        *((char *)bar) = clamp_int_to_char(atoi(fargs[2]));
        break;
      case TYPE_SHORT:
        *((short *)bar) = clamp_int_to_short(atoi(fargs[2]));
        break;
      case TYPE_INT:
        *((int *)bar) = atoi(fargs[2]);
        break;
      case TYPE_FLOAT:
        *((float *)bar) = strtof(fargs[2], nullptr);
        break;
      case TYPE_BV:
        *((int *)bar) = text2bv(fargs[2]);
        break;
      case TYPE_CBV:
        *((byte *)bar) = clamp_int_to_unsigned_char(text2bv(fargs[2]));
        break;
      }
      safe_tprintf_str(buff, bufc, "1");
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

static char *retrieve_value(void *data, int i, char *buffer) {
  if (xcode_data[i].source_kind == GMV_SOURCE_MECH_KEY)
    return mech_value_read_text(data, &xcode_data[i], buffer);

  void *bar = xcode_data[i].source_kind == GMV_SOURCE_FIELD_OFFSET
                  ? (char *)data + xcode_data[i].source.field_offset
                  : nullptr;

  switch (xcode_data[i].type) {
  case TYPE_STRFUNC:
    snprintf(buffer, LBUF_SIZE, "%s",
             xcode_data[i].source.string_callback(0, (Mech *)data));
    break;
  case TYPE_STRFUNC_BD:
    snprintf(
        buffer, LBUF_SIZE, "%s",
        xcode_data[i].source.bidirectional_callback(0, (Mech *)data, nullptr));
    break;
  case TYPE_STRFUNC_BUF:
    xcode_data[i].source.buffered_callback((Mech *)data, buffer);
    break;
  case TYPE_STRFUNC_BD_BUF:
    xcode_data[i].source.buffered_bidirectional_callback(0, (Mech *)data,
                                                         nullptr, buffer);
    break;
  case TYPE_STRING:
    snprintf(buffer, LBUF_SIZE, "%s", (char *)bar);
    break;
  case TYPE_DBREF:
  case TYPE_DBREF_RO:
    snprintf(buffer, LBUF_SIZE, "%ld", (DbRef) * ((DbRef *)bar));
    break;
  case TYPE_CHAR:
  case TYPE_CHAR_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (char)*((char *)bar));
    break;
  case TYPE_SHORT:
  case TYPE_SHORT_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (short)*((short *)bar));
    break;
  case TYPE_INT:
  case TYPE_INT_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (int)*((int *)bar));
    break;
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO:
    snprintf(buffer, LBUF_SIZE, "%.2f", (double)*((float *)bar));
    break;
  case TYPE_BV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text((int)*((int *)bar), (char[SBUF_SIZE]){0}));
    break;
  case TYPE_CBV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text((int)*((char *)bar), (char[SBUF_SIZE]){0}));
    break;
  }
  return buffer;
}

void fun_btgetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = name of the value
   */
  DbRef it;
  int i;
  void *foo;
  int spec;

  it = match_thing(&context->command->match, player, fargs[0]);
  if (it == NOTHING || !is_examinable(context->world->database, player, it)) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  spec = btech_context_which_special(context->btech, it);
  if (!(foo = btech_context_find_object(context->btech, it))) {
    safe_tprintf_str(buff, bufc, "#-1");
    return;
  }
  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 1)) {
      safe_tprintf_str(buff, bufc, "%s",
                       retrieve_value(foo, i, (char[LBUF_SIZE]){0}));
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void fun_btgetxcodevalue_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  /* fargs[0] = mech ref
     fargs[1] = name of the value
   */
  int i;
  Mech *foo;
  int spec;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((foo = load_refmech(context->btech, fargs[0])) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  spec = GTYPE_MECH;
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 1)) {
      safe_tprintf_str(buff, bufc, "%s",
                       retrieve_value(foo, i, (char[LBUF_SIZE]){0}));
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void set_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  char *args[2];
  int t, i;
  void *bar;

  memset(args, 0, sizeof(char *) * 2);

  if (silly_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid arguments!");
    return;
  }
  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  for (i = 0; xcode_data[i].name; i++)
    if (xcode_data[i].gtype == t)
      break;
  if (!xcode_data[i].name) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: No xcode values for this type of object found.");
    return;
  }
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(args[0], xcode_data[i].name) && xcode_data[i].gtype == t &&
        (scode_in_out[xcode_data[i].type] & 2))
      break;
  if (!xcode_data[i].name) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Error: No matching xcode value for this type of object found.");
    return;
  }
  if (xcode_data[i].source_kind == GMV_SOURCE_MECH_KEY) {
    if (!mech_value_write_text(
            btech_context_get_mech(
                context, game_object_location(context->database, player)),
            &xcode_data[i], args[1])) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Error: Unable to set that xcode value.");
      return;
    }
    return;
  }
  bar = xcode_data[i].source_kind == GMV_SOURCE_FIELD_OFFSET
            ? (char *)btech_context_find_object(
                  context, game_object_location(context->database, player)) +
                  xcode_data[i].source.field_offset
            : nullptr;
  switch (xcode_data[i].type) {
  case TYPE_STRFUNC_BD:
  case TYPE_STRFUNC_S:
    xcode_data[i].source.bidirectional_callback(
        1,
        btech_context_get_mech(context,
                               game_object_location(context->database, player)),
        (char *)args[1]);
    break;
  case TYPE_STRFUNC_BD_BUF:
    xcode_data[i].source.buffered_bidirectional_callback(
        1,
        btech_context_get_mech(context,
                               game_object_location(context->database, player)),
        (char *)args[1], (char[LBUF_SIZE]){0});
    break;
  case TYPE_STRING: {
    const size_t capacity = (size_t)xcode_data[i].size;
    if (capacity == 0)
      break;
    strncpy(bar, args[1], capacity - 1);
    ((char *)bar)[capacity - 1] = '\0';
    break;
  }
  case TYPE_DBREF:
    *((DbRef *)bar) = atoi(args[1]);
    break;
  case TYPE_CHAR:
    *((char *)bar) = clamp_int_to_char(atoi(args[1]));
    break;
  case TYPE_SHORT:
    *((short *)bar) = clamp_int_to_short(atoi(args[1]));
    break;
  case TYPE_INT:
    *((int *)bar) = atoi(args[1]);
    break;
  case TYPE_FLOAT:
    *((float *)bar) = strtof(args[1], nullptr);
    break;
  case TYPE_BV:
    *((int *)bar) = text2bv(args[1]);
    break;
  case TYPE_CBV:
    *((byte *)bar) = clamp_int_to_unsigned_char(text2bv(args[1]));
  }
}

void list_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  int t, i, flag = CM_TWO, se_len = 37;
  CoolMenu *c = NULL;

  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  for (i = 0; xcode_data[i].name; i++)
    if (xcode_data[i].gtype == t && (scode_in_out[xcode_data[i].type] & 1))
      break;
  if (!xcode_data[i].name) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: No xcode values for this type of object found.");
    return;
  }
  cool_menu_add_line(&c);
  cool_menu_add_centered(
      &c,
      tprintf("Data for %s (%s)",
              game_object_name(context->database,
                               game_object_location(context->database, player)),
              btech_special_object_type_name(t)));
  cool_menu_add_line(&c);
  if (*buffer == '1') {
    flag = CM_ONE;
    se_len = se_len * 2;
  };
  if (*buffer == '4') {
    flag = CM_FOUR;
    se_len = se_len / 2;
  };
  if (*buffer == '1' || *buffer == '4')
    buffer++;
  for (i = 0; xcode_data[i].name; i++) {
    if (xcode_data[i].gtype == t && (scode_in_out[xcode_data[i].type] & 1)) {
      /* 1/3(left) = name, 2/3(right)=value */
      char mask[SBUF_SIZE];
      char lab[SBUF_SIZE];

      if (*buffer)
        if (strncasecmp(xcode_data[i].name, buffer, strlen(buffer)))
          continue;
      strcpy(lab, xcode_data[i].name);
      lab[se_len / 3] = 0;
      snprintf(mask, SBUF_SIZE, "%%-%ds%%%ds", se_len / 3, se_len * 2 / 3);
      /* mask is built above from a fixed pattern, not external input. */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
#endif
      cool_menu_add_with_flags(
          &c, tprintf(mask, lab, retrieve_value(data, i, (char[LBUF_SIZE]){0})),
          flag);
#ifdef __clang__
#pragma clang diagnostic pop
#endif
    }
  }
  cool_menu_add_line(&c);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

// NOLINTEND(clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference)
