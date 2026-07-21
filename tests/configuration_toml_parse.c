/* configuration_toml_parse.c -- TOML configuration loader unit test */

#include <stdio.h>
#include <string.h>

#include "mux/server/configuration_toml.h"

typedef struct {
  char pname[64];
  char args[256];
} RecordedCall;

typedef struct {
  RecordedCall calls[64];
  int count;
} CallLog;

static int recording_set_fn(const char *pname, const char *args, void *ctx) {
  CallLog *log = ctx;

  if (log->count < 64) {
    snprintf(log->calls[log->count].pname, sizeof(log->calls[0].pname), "%s",
             pname);
    snprintf(log->calls[log->count].args, sizeof(log->calls[0].args), "%s",
             args);
  }
  log->count++;
  return 0;
}

static int call_log_find(const CallLog *log, const char *pname,
                         const char *args) {
  int i;
  int limit;

  limit = log->count < 64 ? log->count : 64;
  for (i = 0; i < limit; i++) {
    if (!strcmp(log->calls[i].pname, pname) && !strcmp(log->calls[i].args, args))
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
                            "[lua]\n"
                            "directory = \"scripts\"\n"
                            "instruction_limit = 50000\n"
                            "memory_limit = 33554432\n"
                            "[mux]\n"
                            "fork_dump = true\n"
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
  ok = log.count == 11 && call_log_find(&log, "port", "5555") &&
       call_log_find(&log, "mud_name", "Test") &&
       call_log_find(&log, "dump_interval", "900") &&
       call_log_find(&log, "lua_directory", "scripts") &&
       call_log_find(&log, "lua_instruction_limit", "50000") &&
       call_log_find(&log, "lua_memory_limit", "33554432") &&
       call_log_find(&log, "fork_dump", "true") &&
       call_log_find(&log, "default_thing_lua_parent", "thing.lua") &&
       call_log_find(&log, "default_room_lua_parent", "room.lua") &&
       call_log_find(&log, "default_exit_lua_parent", "exit.lua") &&
       call_log_find(&log, "default_player_lua_parent", "player.lua");
  toml_free(result);
  return ok;
}

static int test_flag_list_dispatch(void) {
  static const char toml[] =
      "[logging]\nlog = [\"!accounting\", \"bugs\"]\n"
      "[mux]\ndefault_player_flags = []\n"
      "default_exit_flags = [\"no_command\"]\n"
      "default_room_flags = [\"inherit\"]\n"
      "default_thing_flags = [\"safe\"]\n"
      "[flags]\nrobot = [\"robot\"]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 6 && call_log_find(&log, "log", "!accounting bugs") &&
       call_log_find(&log, "default_player_flags", "") &&
       call_log_find(&log, "default_exit_flags", "no_command") &&
       call_log_find(&log, "default_room_flags", "inherit") &&
       call_log_find(&log, "default_thing_flags", "safe") &&
       call_log_find(&log, "robot_flags", "robot");
  toml_free(result);
  return ok;
}

static int test_alias_map_dispatch(void) {
  static const char toml[] = "[aliases.commands]\n\"@ch\" = \"@chown\"\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 1 && call_log_find(&log, "alias", "@ch @chown");
  toml_free(result);
  return ok;
}

static int test_access_map_dispatch(void) {
  static const char toml[] = "[access.functions]\n"
                            "encrypt = \"wizard\"\n"
                            "set = [\"wizard\", \"god\"]\n";
  toml_result_t result;
  CallLog log = {0};
  int ok;

  result = toml_parse(toml, sizeof(toml) - 1);
  if (!result.ok)
    return 0;
  configuration_toml_walk(result.toptab, recording_set_fn, &log);
  ok = log.count == 2 &&
       call_log_find(&log, "function_access", "encrypt wizard") &&
       call_log_find(&log, "function_access", "set wizard god");
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
  ok = log.count == 2 &&
       !strcmp(log.calls[0].pname, "forbid_site") &&
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
  int i;
  const char *port_args = nullptr;
  int saw_function_recursion_limit = 0;

  snprintf(path, sizeof(path), "%s/main.toml", fixture_dir);
  ok = configuration_toml_load(path, recording_set_fn, &log, errbuf,
                              sizeof(errbuf));
  if (!ok)
    return 0;
  for (i = 0; i < log.count && i < 64; i++) {
    if (!strcmp(log.calls[i].pname, "port"))
      port_args = log.calls[i].args;
    if (!strcmp(log.calls[i].pname, "function_recursion_limit"))
      saw_function_recursion_limit = 1;
  }
  /* main.toml's own port (1111) must win over extra.toml's (2222); a key
   * only present in extra.toml must still come through. */
  return port_args != nullptr && !strcmp(port_args, "1111") &&
        saw_function_recursion_limit;
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
  if (argc != 2) {
    fprintf(stderr, "usage: %s <fixture-directory>\n", argv[0]);
    return 1;
  }

  if (!test_scalar_dispatch())
    return 2;
  if (!test_flag_list_dispatch())
    return 3;
  if (!test_alias_map_dispatch())
    return 4;
  if (!test_access_map_dispatch())
    return 5;
  if (!test_site_list_dispatch())
    return 6;
  if (!test_string_list_dispatch())
    return 7;
  if (!test_nested_container_recursion())
    return 8;
  if (!test_unmapped_key_skipped())
    return 9;
  if (!test_include_override_and_merge(argv[1]))
    return 10;
  if (!test_malformed_toml_fails(argv[1]))
    return 11;
  if (!test_missing_include_fails(argv[1]))
    return 12;
  if (!test_include_cycle_fails(argv[1]))
    return 13;
  return 0;
}
