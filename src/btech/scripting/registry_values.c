#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "coolmenu.h"
#include "mech_script_value_api.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "registry_api.h"

#include "checked_conversion.h"

// The static value catalog guarantees source-kind/type pairings. Clang's
// analyzer cannot infer that a field-only type always has a field offset.
// NOLINTBEGIN(clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference)
#include "mux/support/stringutil.h"
#include "script_functions_api.h"
#include "special_object.h"
#include "value_handlers_api.h"
#include "values_internal.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

/* INDENT OFF */
const int SCODE_IN_OUT[TYPE_LAST_TYPE] =
    /* st ch sh in fl db sf sfb sfs bv sfbd cbv ro-ch ro-sh ro-in ro-fl ro-db*/
    {3, 3, 3, 3, 3, 3, 1, 1, 2, 3, 3, 3, 3, 1, 1, 1, 1, 1};
/* INDENT ON */

static int descriptor_permissions(int type) {
  return *(const int *)checked_storage_at_const(
      SCODE_IN_OUT, TYPE_LAST_TYPE, sizeof(*SCODE_IN_OUT), (size_t)type);
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
  MechScriptValue value = {};

  switch (descriptor->type) {
  case TYPE_STRING:
    value.string = text;
    break;
  case TYPE_DBREF:
    if (!parse_long_checked(text, &value.dbref))
      return false;
    break;
  case TYPE_FLOAT:
    if (!parse_float_checked(text, &value.floating))
      return false;
    break;
  case TYPE_BV:
  case TYPE_CBV:
    value.integer = text2bv(text);
    break;
  default:
    if (!parse_int_checked(text, &value.integer))
      return false;
    break;
  }

  return mech_script_value_write(mech, descriptor->source.mech_key, value);
}

static char *mech_value_read_text(const Mech *mech, const GMV *descriptor,
                                  char *buffer) {
  MechScriptValue value = {};
  if (!mech_script_value_read(mech, descriptor->source.mech_key, &value))
    return nullptr;

  switch (descriptor->type) {
  case TYPE_STRING:
    (void)snprintf(buffer, LBUF_SIZE, "%s", value.string);
    break;
  case TYPE_DBREF:
  case TYPE_DBREF_RO:
    (void)snprintf(buffer, LBUF_SIZE, "%ld", value.dbref);
    break;
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO:
    (void)snprintf(buffer, LBUF_SIZE, "%.2f", (double)value.floating);
    break;
  case TYPE_BV:
  case TYPE_CBV:
    (void)snprintf(buffer, LBUF_SIZE, "%s",
                   bv2text(value.integer, (char[SBUF_SIZE]){0}));
    break;
  default:
    (void)snprintf(buffer, LBUF_SIZE, "%d", value.integer);
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
    descriptor->source.buffered_bidirectional_callback(
        &(GmvBufferedBidirectionalCall){.mode = 1,
                                        .mech = data,
                                        .value = text,
                                        .buffer = (char[LBUF_SIZE]){0}});
    return true;
  case TYPE_STRING: {
    const size_t CAPACITY = (size_t)descriptor->size;
    if (CAPACITY == 0)
      return true;
    char *field = descriptor_field(data, descriptor, CAPACITY);
    (void)snprintf(field, CAPACITY, "%s", text);
    return true;
  }
  case TYPE_DBREF: {
    DbRef value;
    if (!parse_long_checked(text, &value))
      return false;
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_CHAR: {
    int parsed;
    if (!parse_int_checked(text, &parsed))
      return false;
    char value = clamp_int_to_char(parsed);
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_SHORT: {
    int parsed;
    if (!parse_int_checked(text, &parsed))
      return false;
    short value = clamp_int_to_short(parsed);
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_INT:
  case TYPE_BV: {
    int value;
    if (descriptor->type == TYPE_BV)
      value = text2bv(text);
    else if (!parse_int_checked(text, &value))
      return false;
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_FLOAT: {
    float value;
    if (!parse_float_checked(text, &value))
      return false;
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  case TYPE_CBV: {
    Byte value = clamp_int_to_unsigned_char(text2bv(text));
    memcpy(descriptor_field(data, descriptor, sizeof(value)), &value,
           sizeof(value));
    return true;
  }
  default:
    return false;
  }
}

/**
 * Lists live unit objects assigned to a zone.
 *
 * @par LuaLS definition btech callable btech.system.zone_units
 * @code{.lua}
 * ---Lists live unit dbrefs assigned to a zone. A trailing `-1` indicates that the legacy output was truncated.
 * ---@param zone integer
 * ---@return integer[] units Unit dbrefs, possibly followed by the `-1` truncation sentinel.
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_system.zone_units(zone) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_zmechs(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  DbRef it = match_thing(&context->command->match, PLAYER,
                         script_function_argument(fargs, NFARGS, 0));
  DbRef i;
  size_t len = 0;
  char reference[SBUF_SIZE];

  if (!is_controls(context->world->database, PLAYER, it) &&
      !is_wizard(context->btech->database, PLAYER)) {
    return btech_script_error(call, "#-1 NO PERMISSION TO USE");
  }
  for (i = 0; i < context->btech->database->top; i++) {
    if (typeof_obj(context->btech->database, i) == OBJECT_TYPE_THING) {
      if (game_object_zone(context->btech->database, i) == it) {
        if ((btech_context_which_special(context->btech, i) == GTYPE_MECH) &&
            is_good_obj(context->btech->database, i)) {
          if (len) {
            (void)snprintf(reference, sizeof(reference), " #%ld", i);
            if ((strlen(reference) + len) > (LBUF_SIZE - SBUF_SIZE)) {
              safe_str(" #-1", buff, bufc);
              return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
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

  return btech_script_result_finish(call, BTECH_SCRIPT_LIST);
}

/**
 * Writes a script-writable native field on a live special object.
 *
 * @par LuaLS definition btech callable btech.system.set_xcode_value
 * @code{.lua}
 * ---Writes a script-writable native field on a live special object.
 * ---@param object integer
 * ---@param name string Native field name, matched ASCII-case-insensitively.
 * ---@param value string|number
 * ---@return true success
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_system.set_xcode_value(object, name, value) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btsetxcodevalue(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
     script_function_argument(fargs, nfargs, 1) = name of the value
     script_function_argument(fargs, nfargs, 2) = what the value's to be set as
   */
  DbRef it;
  int spec;
  void *foo;

  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  spec = btech_context_which_special(context->btech, it);
  foo = btech_context_find_object(context->btech, it);
  if (!foo) {
    return btech_script_error(call, "#-1");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const GMV *descriptor =
      find_descriptor(script_function_argument(fargs, NFARGS, 1), spec, 2);
  if (descriptor != nullptr &&
      descriptor_write_text(foo, descriptor,
                            script_function_argument(fargs, NFARGS, 2))) {
    safe_tprintf_str(buff, bufc, "1");
    return btech_script_result_finish(call, BTECH_SCRIPT_MUTATION);
  }
  return btech_script_error(call, "#-1");
}

static char *retrieve_value(void *data, const GMV *descriptor, char *buffer) {
  if (descriptor->source_kind == GMV_SOURCE_MECH_KEY)
    return mech_value_read_text(data, descriptor, buffer);

  switch (descriptor->type) {
  case TYPE_STRFUNC:
    (void)snprintf(buffer, LBUF_SIZE, "%s",
                   descriptor->source.string_callback(0, data));
    break;
  case TYPE_STRFUNC_BD:
    (void)snprintf(buffer, LBUF_SIZE, "%s",
                   descriptor->source.bidirectional_callback(0, data, nullptr));
    break;
  case TYPE_STRFUNC_BUF:
    descriptor->source.buffered_callback(data, buffer);
    break;
  case TYPE_STRFUNC_BD_BUF:
    descriptor->source.buffered_bidirectional_callback(
        &(GmvBufferedBidirectionalCall){
            .mech = data, .value = nullptr, .buffer = buffer});
    break;
  case TYPE_STRING: {
    const char *field =
        descriptor_field_const(data, descriptor, (size_t)descriptor->size);
    (void)snprintf(buffer, LBUF_SIZE, "%s", field);
    break;
  }
  case TYPE_DBREF:
  case TYPE_DBREF_RO: {
    DbRef value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    (void)snprintf(buffer, LBUF_SIZE, "%ld", value);
    break;
  }
  case TYPE_CHAR:
  case TYPE_CHAR_RO: {
    char value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    (void)snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_SHORT:
  case TYPE_SHORT_RO: {
    short value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    (void)snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_INT:
  case TYPE_INT_RO:
  case TYPE_BV: {
    int value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    if (descriptor->type == TYPE_BV)
      (void)snprintf(buffer, LBUF_SIZE, "%s",
                     bv2text(value, (char[SBUF_SIZE]){0}));
    else
      (void)snprintf(buffer, LBUF_SIZE, "%d", value);
    break;
  }
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO: {
    float value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    (void)snprintf(buffer, LBUF_SIZE, "%.2f", (double)value);
    break;
  }
  case TYPE_CBV: {
    Byte value;
    memcpy(&value, descriptor_field_const(data, descriptor, sizeof(value)),
           sizeof(value));
    (void)snprintf(buffer, LBUF_SIZE, "%s",
                   bv2text((int)value, (char[SBUF_SIZE]){0}));
    break;
  }
  }
  return buffer;
}

/**
 * Reads a script-visible native field from a live special object.
 *
 * @par LuaLS definition btech callable btech.system.xcode_value
 * @code{.lua}
 * ---Reads a script-visible native field from a live special object.
 * ---@param object integer
 * ---@param name string Native field name, matched ASCII-case-insensitively.
 * ---@return string result
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_system.xcode_value(object, name) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btgetxcodevalue(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = id of the mech
     script_function_argument(fargs, nfargs, 1) = name of the value
   */
  DbRef it;
  void *foo;
  int spec;

  it = match_thing(&context->command->match, PLAYER,
                   script_function_argument(fargs, NFARGS, 0));
  if (it == NOTHING || !is_examinable(context->world->database, PLAYER, it)) {
    return btech_script_error(call, "#-1");
  }
  spec = btech_context_which_special(context->btech, it);
  foo = btech_context_find_object(context->btech, it);
  if (!foo) {
    return btech_script_error(call, "#-1");
  }
  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  const GMV *descriptor =
      find_descriptor(script_function_argument(fargs, NFARGS, 1), spec, 1);
  if (descriptor != nullptr) {
    safe_tprintf_str(buff, bufc, "%s",
                     retrieve_value(foo, descriptor, (char[LBUF_SIZE]){0}));
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  return btech_script_error(call, "#-1");
}

/**
 * Reads a script-visible native field from a unit template.
 *
 * @par LuaLS definition btech callable btech.system.xcode_value_ref
 * @code{.lua}
 * ---Reads a script-visible native field from a unit template.
 * ---@param reference string
 * ---@param name string Native field name, matched ASCII-case-insensitively.
 * ---@return string result
 * ---
 * ---Raises [`btech.error.codes.unavailable`](lua://btech.error.codes.unavailable), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`btech.error.codes.failed`](lua://btech.error.codes.failed).
 * ---@see btech.error.codes.unavailable
 * ---@see mux.error.codes.arg.invalid
 * ---@see btech.error.codes.failed
 * function btech_system.xcode_value_ref(reference, name) end
 * @endcode
 * @param[in,out] call The BattleTech arguments, output, and evaluation context.
 * @return A `BtechScriptResult` consumed by the Lua trampoline.
 */
BtechScriptResult fun_btgetxcodevalue_ref(BtechScriptCall *call) {
  [[maybe_unused]] char *buff = call->output.buffer;
  [[maybe_unused]] char **bufc = &call->output.cursor;
  [[maybe_unused]] char **fargs = call->arguments.values;
  [[maybe_unused]] const int NFARGS = (int)call->arguments.count;
  [[maybe_unused]] char **cargs = call->command_arguments.values;
  [[maybe_unused]] const int NCARGS = (int)call->command_arguments.count;
  [[maybe_unused]] EvaluationContext *context = call->evaluation;
  [[maybe_unused]] const DbRef PLAYER = call->player;
  /* script_function_argument(fargs, nfargs, 0) = mech ref
     script_function_argument(fargs, nfargs, 1) = name of the value
   */
  Mech *foo;

  if (!is_wizard(context->world->database, PLAYER)) {
    return btech_script_error(call, "#-1 PERMISSION DENIED");
  }
  foo =
      load_refmech(context->btech, script_function_argument(fargs, NFARGS, 0));
  if (!foo) {
    return btech_script_error(call, "#-1 NO SUCH MECH");
  }
  const GMV *descriptor = find_descriptor(
      script_function_argument(fargs, NFARGS, 1), GTYPE_MECH, 1);
  if (descriptor != nullptr) {
    safe_tprintf_str(buff, bufc, "%s",
                     retrieve_value(foo, descriptor, (char[LBUF_SIZE]){0}));
    return btech_script_result_finish(call, BTECH_SCRIPT_TEXT);
  }
  return btech_script_error(call, "#-1");
}

void set_xcodestuff(DbRef player, BtechSpecialObject *object, char *buffer) {
  BtechContext *context = object->context;
  char *args[2];
  int t;

  memset((void *)args, 0, sizeof(char *) * 2);

  if (silly_parseattributes(buffer, args, 2) != 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid arguments!");
    return;
  }
  char *name = *(char **)checked_storage_at((void *)args, 2, sizeof(*args), 0);
  char *value = *(char **)checked_storage_at((void *)args, 2, sizeof(*args), 1);
  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  if (find_descriptor(nullptr, t, 1) == nullptr) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: No xcode values for this type of object found.");
    free_text_items(args, 2);
    return;
  }
  const GMV *descriptor = find_descriptor(name, t, 2);
  if (descriptor == nullptr) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Error: No matching xcode value for this type of object found.");
    free_text_items(args, 2);
    return;
  }
  void *target_object = btech_context_find_object(
      context, game_object_location(context->database, player));
  if (!descriptor_write_text(target_object, descriptor, value))
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: Unable to set that xcode value.");
  free_text_items(args, 2);
}

void list_xcodestuff(DbRef player, BtechSpecialObject *object,
                     const char *buffer) {
  BtechContext *context = object->context;
  int t;
  int flag = CM_TWO;
  int se_len = 37;
  CoolMenu *c = nullptr;

  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  if (find_descriptor(nullptr, t, 1) == nullptr) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Error: No xcode values for this type of object found.");
    return;
  }
  char *message_buffer = alloc_lbuf("list_xcodestuff.message");
  char *value_buffer = alloc_lbuf("list_xcodestuff.value");
  cool_menu_add_line(&c);
  (void)snprintf(
      message_buffer, LBUF_SIZE, "Data for %s (%s)",
      game_object_name(context->database,
                       game_object_location(context->database, player)),
      btech_special_object_type_name(t));
  cool_menu_add_centered(&c, message_buffer);
  cool_menu_add_line(&c);
  if (*buffer == '1') {
    flag = CM_ONE;
    se_len = se_len * 2;
  }
  if (*buffer == '4') {
    flag = CM_FOUR;
    se_len = se_len / 2;
  }
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
      (void)snprintf(lab, sizeof(lab), "%s", descriptor->name);
      const size_t LABEL_LIMIT = (size_t)(se_len / 3);
      *(char *)checked_storage_at(lab, sizeof(lab), sizeof(char), LABEL_LIMIT) =
          '\0';
      (void)snprintf(message_buffer, LBUF_SIZE, "%-*s%*s", se_len / 3, lab,
                     se_len * 2 / 3,
                     retrieve_value(object, descriptor, value_buffer));
      cool_menu_add_with_flags(&c, message_buffer, flag);
    }
  }
  cool_menu_add_line(&c);
  show_cool_menu(btech_context_evaluation(context), player, c);
  kill_cool_menu(c);
  free_buf(value_buffer);
  free_buf(message_buffer);
}

// NOLINTEND(clang-analyzer-core.NonNullParamChecker,clang-analyzer-core.NullDereference)
