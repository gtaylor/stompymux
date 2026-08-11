/* configuration_toml_parse.c -- TOML configuration loader unit test */

#include <stdio.h>
#include <string.h>

#include "mux/server/configuration_toml.h"
#include "mux/support/checked_storage.h"

typedef struct {
  char pname[64];
  char args[256];
} RecordedCall;

typedef struct {
  RecordedCall calls[64];
  size_t count;
} CallLog;

static RecordedCall *call_log_slot(CallLog *log, size_t index) {
  return checked_storage_at(log->calls,
                            sizeof(log->calls) / sizeof(log->calls[0]),
                            sizeof(*log->calls), index);
}

static const RecordedCall *call_log_item(const CallLog *log, size_t index) {
  return checked_storage_at_const(log->calls,
                                  sizeof(log->calls) / sizeof(log->calls[0]),
                                  sizeof(*log->calls), index);
}

static int recording_set_fn(const char *pname, const char *args, void *ctx) {
  CallLog *log = ctx;

  if (log->count < 64) {
    RecordedCall *call = call_log_slot(log, log->count);

    snprintf(call->pname, sizeof(call->pname), "%s", pname);
    snprintf(call->args, sizeof(call->args), "%s", args);
  }
  log->count++;
  return 0;
}

static int call_log_find(const CallLog *log, const char *pname,
                         const char *args) {
  size_t i;
  size_t limit;

  limit = log->count < 64 ? log->count : 64;
  for (i = 0; i < limit; i++) {
    const RecordedCall *call = call_log_item(log, i);

    if (!strcmp(call->pname, pname) && !strcmp(call->args, args))
      return 1;
  }
  return 0;
}

static int test_scalar_dispatch(void) {
  static const char toml[] = "[server]\n"
                             "port = 5555\n"
                             "mud_name = \"Test\"\n"
                             "[database]\n"
                             "dump_interval = 900\n"
                             "fork_dump = true\n"
                             "dump_message = \"Saving\"\n"
                             "postdump_message = \"Saved\"\n"
                             "[lua]\n"
                             "directory = \"scripts\"\n"
                             "memory_limit = 33554432\n"
                             "state_value_limit = 16384\n"
                             "state_entry_limit = 256\n"
                             "state_object_limit = 262144\n"
                             "[mux]\n"
                             "default_thing_lua_parent = \"thing.lua\"\n"
                             "default_room_lua_parent = \"room.lua\"\n"
                             "default_exit_lua_parent = \"exit.lua\"\n"
                             "default_player_lua_parent = \"player.lua\"\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 15 && call_log_find(&log, "port", "5555") &&
       call_log_find(&log, "mud_name", "Test") &&
       call_log_find(&log, "dump_interval", "900") &&
       call_log_find(&log, "lua_directory", "scripts") &&
       call_log_find(&log, "lua_memory_limit", "33554432") &&
       call_log_find(&log, "lua_state_value_limit", "16384") &&
       call_log_find(&log, "lua_state_entry_limit", "256") &&
       call_log_find(&log, "lua_state_object_limit", "262144") &&
       call_log_find(&log, "fork_dump", "true") &&
       call_log_find(&log, "dump_message", "Saving") &&
       call_log_find(&log, "postdump_message", "Saved") &&
       call_log_find(&log, "default_thing_lua_parent", "thing.lua") &&
       call_log_find(&log, "default_room_lua_parent", "room.lua") &&
       call_log_find(&log, "default_exit_lua_parent", "exit.lua") &&
       call_log_find(&log, "default_player_lua_parent", "player.lua");
  toml_free(result);
  return ok;
}

static int test_flag_list_and_logging_topics_dispatch(void) {
  static const char toml[] = "[logging]\n"
                             "log_options = [\"flags\", \"location\"]\n"
                             "[logging.topics]\n"
                             "accounting = false\n"
                             "all_commands = false\n"
                             "suspect_commands = false\n"
                             "bad_commands = false\n"
                             "buffer_alloc = false\n"
                             "bugs = true\n"
                             "checkpoints = false\n"
                             "config_changes = false\n"
                             "create = false\n"
                             "logins = false\n"
                             "network = false\n"
                             "problems = true\n"
                             "security = true\n"
                             "shouts = false\n"
                             "startup = false\n"
                             "wizard = false\n"
                             "[mux]\ndefault_player_flags = []\n"
                             "default_exit_flags = [\"no_command\"]\n"
                             "default_room_flags = [\"floating\"]\n"
                             "default_thing_flags = [\"safe\"]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 21 &&
       call_log_find(&log, "log_options", "flags location") &&
       call_log_find(&log, "accounting", "false") &&
       call_log_find(&log, "all_commands", "false") &&
       call_log_find(&log, "suspect_commands", "false") &&
       call_log_find(&log, "bad_commands", "false") &&
       call_log_find(&log, "buffer_alloc", "false") &&
       call_log_find(&log, "bugs", "true") &&
       call_log_find(&log, "checkpoints", "false") &&
       call_log_find(&log, "config_changes", "false") &&
       call_log_find(&log, "create", "false") &&
       call_log_find(&log, "logins", "false") &&
       call_log_find(&log, "network", "false") &&
       call_log_find(&log, "problems", "true") &&
       call_log_find(&log, "security", "true") &&
       call_log_find(&log, "shouts", "false") &&
       call_log_find(&log, "startup", "false") &&
       call_log_find(&log, "wizard", "false") &&
       call_log_find(&log, "default_player_flags", "") &&
       call_log_find(&log, "default_exit_flags", "no_command") &&
       call_log_find(&log, "default_room_flags", "floating") &&
       call_log_find(&log, "default_thing_flags", "safe");
  toml_free(result);
  return ok;
}

static int test_alias_map_dispatch(void) {
  static const char toml[] = "[aliases.commands]\n\"@cr\" = \"@create\"\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 1 && call_log_find(&log, "alias", "@cr @create");
  toml_free(result);
  return ok;
}

static int test_bootstrap_object_map_dispatch(void) {
  static const char toml[] =
      "[database.bootstrap.objects]\n"
      "0 = { type = \"room\", name = \"Limbo\" }\n"
      "1 = { type = \"player\", name = \"GOD\", wizard = true }\n"
      "4 = { type = \"room\", name = \"Starter Room\" }\n";
  toml_result_t result = toml_parse(toml, sizeof(toml) - 1);
  CallLog log = {0};

  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  int ok = log.count == 4 &&
           call_log_find(&log, "bootstrap_objects_clear", "") &&
           call_log_find(&log, "bootstrap_object", "0 room false Limbo") &&
           call_log_find(&log, "bootstrap_object", "1 player true GOD") &&
           call_log_find(&log, "bootstrap_object", "4 room false Starter Room");
  toml_free(result);
  return ok;
}

static int test_rgb_map_dispatch(void) {
  static const char toml[] = "[colors]\n"
                             "brand_red = [205, 0, 0]\n"
                             "brand_blue = [32, 96, 192]\n"
                             "bad_length = [1, 2]\n"
                             "bad_type = [1, \"2\", 3]\n"
                             "bad_range = [1, 2, 256]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 &&
       call_log_find(&log, "named_color", "brand_red 205 0 0") &&
       call_log_find(&log, "named_color", "brand_blue 32 96 192");
  toml_free(result);
  return ok;
}

static int test_osc8_preset_map_dispatch(void) {
  static const char toml[] =
      "[osc8.presets]\n"
      "button = 'color=white bg=green bold'\n"
      "poll = 'selection.group=\"demo\" selection.exclusive'\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 &&
       call_log_find(&log, "osc8_preset", "button color=white bg=green bold") &&
       call_log_find(&log, "osc8_preset",
                     "poll selection.group=\"demo\" selection.exclusive");
  toml_free(result);
  return ok;
}

static int test_access_map_dispatch(void) {
  static const char toml[] = "[access.commands]\n"
                             "\"@dig\" = \"wizard\"\n"
                             "\"@boot\" = [\"wizard\", \"god\"]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 && call_log_find(&log, "access", "@dig wizard") &&
       call_log_find(&log, "access", "@boot wizard god");
  toml_free(result);
  return ok;
}

static int test_site_list_dispatch(void) {
  static const char toml[] =
      "[sites]\nforbid = ["
      "{ address = \"1.2.3.4\", mask = \"255.255.255.0\" },"
      "{ address = \"5.6.7.8\", mask = \"255.255.255.255\" },"
      "]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 && !strcmp(log.calls[0].pname, "forbid_site") &&
       !strcmp(log.calls[0].args, "1.2.3.4 255.255.255.0") &&
       !strcmp(log.calls[1].pname, "forbid_site") &&
       !strcmp(log.calls[1].args, "5.6.7.8 255.255.255.255");
  toml_free(result);
  return ok;
}

static int test_string_list_dispatch(void) {
  static const char toml[] = "[names]\nbad = [\"here\", \"you\"]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 && call_log_find(&log, "bad_name", "here") &&
       call_log_find(&log, "bad_name", "you");
  toml_free(result);
  return ok;
}

static int test_nested_container_recursion(void) {
  static const char toml[] = "[battletech.xp]\nbthmod = 200\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 1 && call_log_find(&log, "btech_xp_bthmod", "200");
  toml_free(result);
  return ok;
}

static int test_unmapped_key_skipped(void) {
  static const char toml[] = "[server]\nport = 5555\nbogus_directive = 1\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 1 && call_log_find(&log, "port", "5555");
  toml_free(result);
  return ok;
}

static int test_include_override_and_merge(const char *fixture_dir) {
  char path[512];
  char errbuf[256];
  CallLog log = {0};
  int ok;
  size_t i;
  const char *port_args = nullptr;
  int saw_dump_interval = 0;
  const char *color_args = nullptr;

  snprintf(path, sizeof(path), "%s/main.toml", fixture_dir);
  ok = configuration_toml_load(path, recording_set_fn, &log, errbuf,
                               sizeof(errbuf));
  if (!ok)
    return 0;
  for (i = 0; i < log.count && i < 64; i++) {
    const RecordedCall *call = call_log_item(&log, i);

    if (!strcmp(call->pname, "port"))
      port_args = call->args;
    if (!strcmp(call->pname, "dump_interval"))
      saw_dump_interval = 1;
    if (!strcmp(call->pname, "named_color"))
      color_args = call->args;
  }
  /* main.toml's own port (1111) must win over extra.toml's (2222); a key
   * only present in extra.toml must still come through. */
  return port_args != nullptr && !strcmp(port_args, "1111") &&
         saw_dump_interval && color_args != nullptr &&
         !strcmp(color_args, "brand 10 20 30");
}

static int test_malformed_toml_fails(const char *fixture_dir) {
  char path[512];
  char errbuf[256];
  CallLog log = {0};
  int ok;

  snprintf(path, sizeof(path), "%s/malformed.toml", fixture_dir);
  ok = configuration_toml_load(path, recording_set_fn, &log, errbuf,
                               sizeof(errbuf));
  return !ok && errbuf[0];
}

static int test_missing_include_fails(const char *fixture_dir) {
  char path[512];
  char errbuf[256];
  CallLog log = {0};
  int ok;

  snprintf(path, sizeof(path), "%s/missing_include.toml", fixture_dir);
  ok = configuration_toml_load(path, recording_set_fn, &log, errbuf,
                               sizeof(errbuf));
  return !ok && errbuf[0];
}

static int test_include_cycle_fails(const char *fixture_dir) {
  char path[512];
  char errbuf[256];
  CallLog log = {0};
  int ok;

  snprintf(path, sizeof(path), "%s/cycle_a.toml", fixture_dir);
  ok = configuration_toml_load(path, recording_set_fn, &log, errbuf,
                               sizeof(errbuf));
  return !ok && errbuf[0];
}

int main(int argc, char *argv[]) {
  const char *fixture_dir;

  if (argc != 2) {
    fprintf(stderr, "usage: %s <fixture-directory>\n", argv[0]);
    return 1;
  }
  fixture_dir = *(char *const *)checked_storage_at_const(argv, (size_t)argc,
                                                         sizeof(*argv), 1);

  if (!test_scalar_dispatch())
    return 2;
  if (!test_flag_list_and_logging_topics_dispatch())
    return 3;
  if (!test_alias_map_dispatch())
    return 4;
  if (!test_bootstrap_object_map_dispatch())
    return 16;
  if (!test_rgb_map_dispatch())
    return 5;
  if (!test_osc8_preset_map_dispatch())
    return 15;
  if (!test_access_map_dispatch())
    return 6;
  if (!test_site_list_dispatch())
    return 7;
  if (!test_string_list_dispatch())
    return 8;
  if (!test_nested_container_recursion())
    return 9;
  if (!test_unmapped_key_skipped())
    return 10;
  if (!test_include_override_and_merge(fixture_dir))
    return 11;
  if (!test_malformed_toml_fails(fixture_dir))
    return 12;
  if (!test_missing_include_fails(fixture_dir))
    return 13;
  if (!test_include_cycle_fails(fixture_dir))
    return 14;
  return 0;
}
