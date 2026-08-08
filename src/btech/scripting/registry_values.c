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

static int descriptor_permissions(int type) {
  return *(const int *)checked_storage_at_const(
      scode_in_out, TYPE_LAST_TYPE, sizeof(*scode_in_out), (size_t)type);
}

static const GMV *find_descriptor(const char *name, int special_type,
                                  int required_permission) {
  for (size_t index = 0; index < xcode_descriptor_count(); ++index) {
    const GMV *descriptor = xcode_descriptor_at(index);
    if (descriptor->gtype == special_type &&
        (descriptor_permissions(descriptor->type) & required_permission) &&
        (name == nullptr || strcasecmp(name, descriptor->name) == 0))
      return descriptor;
  }
  return nullptr;
}

static void *descriptor_field(void *data, const GMV *descriptor,
                              size_t field_size) {
  return checked_storage_region(
      data, btech_special_object_storage_size(descriptor->gtype),
      descriptor->source.field_offset, field_size);
}

static const void *descriptor_field_const(const void *data,
                                          const GMV *descriptor,
                                          size_t field_size) {
  return checked_storage_region_const(
      data, btech_special_object_storage_size(descriptor->gtype),
      descriptor->source.field_offset, field_size);
}

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

static bool descriptor_write_text(void *data, const GMV *descriptor,
                                  char *text) {
  if (descriptor->source_kind == GMV_SOURCE_MECH_KEY)
    return mech_value_write_text(data, descriptor, text);

  switch (descriptor->type) {
  case TYPE_STRFUNC_BD:
  case TYPE_STRFUNC_S:
    descriptor->source.bidirectional_callback(1, data, text);
    return true;
  case TYPE_STRFUNC_BD_BUF:
    descriptor->source.buffered_bidirectional_callback(1, data, text,
                                                       (char[LBUF_SIZE]){0});
    return true;
  case TYPE_STRING: {
    const size_t capacity = (size_t)descriptor->size;
    if (capacity == 0)
      return true;
    char *field = descriptor_field(data, descriptor, capacity);
    snprintf(field, capacity, "%s", text);
    return true;
  }
  case TYPE_DBREF: {
    DbRef value = atoi(text);
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_CHAR: {
    char value = clamp_int_to_char(atoi(text));
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_SHORT: {
    short value = clamp_int_to_short(atoi(text));
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_INT:
  case TYPE_BV: {
    int value = descriptor->type == TYPE_BV ? text2bv(text) : atoi(text);
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_FLOAT: {
    float value = strtof(text, nullptr);
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_CBV: {
    byte value = clamp_int_to_unsigned_char(text2bv(text));
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  default:
    return false;
  }
}

void fun_zmechs(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player,
                         script_function_argument(fargs, nfargs, 0));
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
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
     script_function_argument(fargs, nfargs, 1) = name of the value
     script_function_argument(fargs, nfargs, 2) = what the value's to be set as
   */
  DbRef it;
  int spec;
  void *foo;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  const GMV *descriptor =
      find_descriptor(script_function_argument(fargs, nfargs, 1), spec, 2);
  if (descriptor != nullptr &&
      descriptor_write_text(foo, descriptor,
                            script_function_argument(fargs, nfargs, 2))) {
    safe_tprintf_str(buff, bufc, "1");
    return;
  }
  safe_tprintf_str(buff, bufc, "#-1");
}

static char *retrieve_value(void *data, const GMV *descriptor, char *buffer) {
  if (descriptor->source_kind == GMV_SOURCE_MECH_KEY)
    return mech_value_read_text(data, descriptor, buffer);

  switch (descriptor->type) {
  case TYPE_STRFUNC:
    snprintf(buffer, LBUF_SIZE, "%s",
             descriptor->source.string_callback(0, (Mech *)data));
    break;
  case TYPE_STRFUNC_BD:
    snprintf(
        buffer, LBUF_SIZE, "%s",
        descriptor->source.bidirectional_callback(0, (Mech *)data, nullptr));
    break;
  case TYPE_STRFUNC_BUF:
    descriptor->source.buffered_callback((Mech *)data, buffer);
    break;
  case TYPE_STRFUNC_BD_BUF:
    descriptor->source.buffered_bidirectional_callback(0, (Mech *)data, nullptr,
                                                       buffer);
    break;
  case TYPE_STRING: {
    const char *field =
        descriptor_field_const(data, descriptor, (size_t)descriptor->size);
    snprintf(buffer, LBUF_SIZE, "%s", field);
    break;
  }
  case TYPE_DBREF:
  case TYPE_DBREF_RO: {
    DbRef value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    snprintf(buffer, LBUF_SIZE, "%ld", value);
    break;
  }
  case TYPE_CHAR:
  case TYPE_CHAR_RO: {
    char value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_SHORT:
  case TYPE_SHORT_RO: {
    short value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_INT:
  case TYPE_INT_RO:
  case TYPE_BV: {
    int value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    if (descriptor->type == TYPE_BV)
      snprintf(buffer, LBUF_SIZE, "%s", bv2text(value, (char[SBUF_SIZE]){0}));
    else
      snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO: {
    float value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    snprintf(buffer, LBUF_SIZE, "%.2f", (double)value);
    break;
  }
  case TYPE_CBV: {
    byte value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text((int)value, (char[SBUF_SIZE]){0}));
    break;
  }
  }
  return buffer;
}

void fun_btgetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
     script_function_argument(fargs, nfargs, 1) = name of the value
   */
  DbRef it;
  void *foo;
  int spec;

  it = match_thing(&context->command->match, player,
                   script_function_argument(fargs, nfargs, 0));
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
  const GMV *descriptor =
      find_descriptor(script_function_argument(fargs, nfargs, 1), spec, 1);
  if (descriptor != nullptr) {
    safe_tprintf_str(buff, bufc, "%s",
                     retrieve_value(foo, descriptor, (char[LBUF_SIZE]){0}));
    return;
  }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void fun_btgetxcodevalue_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  /* script_function_argument(fargs, nfargs, 0) = mech ref
     script_function_argument(fargs, nfargs, 1) = name of the value
   */
  Mech *foo;

  if (!is_wizard(context->world->database, player)) {
    safe_tprintf_str(buff, bufc, "#-1 PERMISSION DENIED");
    return;
  }
  if ((foo = load_refmech(context->btech, script_function_argument(
                                              fargs, nfargs, 0))) == NULL) {
    safe_tprintf_str(buff, bufc, "#-1 NO SUCH MECH");
    return;
  }
  const GMV *descriptor = find_descriptor(
      script_function_argument(fargs, nfargs, 1), GTYPE_MECH, 1);
  if (descriptor != nullptr) {
    safe_tprintf_str(buff, bufc, "%s",
                     retrieve_value(foo, descriptor, (char[LBUF_SIZE]){0}));
    return;
  }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void set_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  char *args[2];
  int t;

  memset(args, 0, sizeof(char *) * 2);

  if (silly_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid arguments!");
    return;
  }
  char *name = *(char **)checked_storage_at(args, 2, sizeof(*args), 0);
  char *value = *(char **)checked_storage_at(args, 2, sizeof(*args), 1);
  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  if (find_descriptor(nullptr, t, 1) == nullptr) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: No xcode values for this type of object found.");
    FreeTextItems(args, 2);
    return;
  }
  const GMV *descriptor = find_descriptor(name, t, 2);
  if (descriptor == nullptr) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Error: No matching xcode value for this type of object found.");
    FreeTextItems(args, 2);
    return;
  }
  void *object = btech_context_find_object(
      context, game_object_location(context->database, player));
  if (!descriptor_write_text(object, descriptor, value))
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: Unable to set that xcode value.");
  FreeTextItems(args, 2);
}

void list_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  int t, flag = CM_TWO, se_len = 37;
  CoolMenu *c = NULL;

  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  if (find_descriptor(nullptr, t, 1) == nullptr) {
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
  const char *filter = buffer;
  if (*filter == '1' || *filter == '4')
    filter = checked_string_suffix(filter, 1);
  for (size_t index = 0; index < xcode_descriptor_count(); ++index) {
    const GMV *descriptor = xcode_descriptor_at(index);
    if (descriptor->gtype == t &&
        (descriptor_permissions(descriptor->type) & 1)) {
      /* 1/3(left) = name, 2/3(right)=value */
      char lab[SBUF_SIZE];

      if (*filter)
        if (strncasecmp(descriptor->name, filter, strlen(filter)))
          continue;
      snprintf(lab, sizeof(lab), "%s", descriptor->name);
      const size_t label_limit = (size_t)(se_len / 3);
      *(char *)checked_storage_at(lab, sizeof(lab), sizeof(char), label_limit) =
          '\0';
      cool_menu_add_with_flags(
          &c,
          tprintf("%-*s%*s", se_len / 3, lab, se_len * 2 / 3,
                  retrieve_value(data, descriptor, (char[LBUF_SIZE]){0})),
          flag);
    }
  }
  cool_menu_add_line(&c);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}

// NOLINTEND(clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference)
