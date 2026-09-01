/* Process-level smoke test for the libuv TCP listener and shutdown path. */

#include <arpa/inet.h>
#include <errno.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
#include <sqlite3.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "mux/support/checked_storage.h"

/* Number of simultaneous clients used to exercise descriptor registry growth.
 */
constexpr size_t TEST_CONNECTION_COUNT = 20;
constexpr unsigned char TELNET_IAC = 255;
constexpr unsigned char TELNET_SB = 250;
constexpr unsigned char TELNET_SE = 240;
constexpr unsigned char TELNET_WILL = 251;
constexpr unsigned char TELNET_DO = 253;
constexpr unsigned char TELNET_TTYPE = 24;
constexpr unsigned char TELNET_TTYPE_IS = 0;
constexpr unsigned char TELNET_TTYPE_SEND = 1;
constexpr unsigned char TELNET_NEW_ENVIRON = 39;
constexpr unsigned char TELNET_ENVIRON_IS = 0;
constexpr unsigned char TELNET_ENVIRON_SEND = 1;
constexpr unsigned char TELNET_ENVIRON_VAR = 0;
constexpr unsigned char TELNET_ENVIRON_VALUE = 1;
constexpr unsigned char TELNET_ENVIRON_ESC = 2;
constexpr unsigned char TELNET_ENVIRON_USERVAR = 3;
constexpr unsigned char TELNET_CHARSET = 42;
constexpr int TEST_IO_TIMEOUT_MS = 10000;

typedef struct TelnetTestClient {
  int socket_fd;
  unsigned char received[16384];
  size_t received_size;
} TelnetTestClient;

static void *buffer_suffix(void *buffer, size_t capacity, size_t offset) {
  return checked_storage_region(buffer, capacity, offset, capacity - offset);
}

static const void *constant_buffer_suffix(const void *buffer, size_t capacity,
                                          size_t offset) {
  return checked_storage_region_const(buffer, capacity, offset,
                                      capacity - offset);
}

static void append_byte(unsigned char *buffer, size_t capacity, size_t *size,
                        unsigned char value) {
  unsigned char *slot =
      checked_storage_at(buffer, capacity, sizeof(*buffer), *size);
  *slot = value;
  (*size)++;
}

static int *socket_slot(int *sockets, size_t index) {
  return checked_storage_at(sockets, TEST_CONNECTION_COUNT, sizeof(*sockets),
                            index);
}

static char *process_argument(char **arguments, int count, int index) {
  if (count < 0 || index < 0)
    abort();
  return *(char **)checked_storage_at(arguments, (size_t)count,
                                      sizeof(*arguments), (size_t)index);
}

/* Wait for child to exit successfully, killing it after the timeout. */
static int wait_child(pid_t child) {
  struct timespec delay = {.tv_sec = 0, .tv_nsec = 100000000};
  int status;

  for (int attempt = 0; attempt < 30; attempt++) {
    pid_t result = waitpid(child, &status, WNOHANG);

    if (result == child)
      return WIFEXITED(status) && WEXITSTATUS(status) == 0 ? 0 : -1;
    if (result < 0)
      return -1;
    nanosleep(&delay, nullptr);
  }
  kill(child, SIGKILL);
  waitpid(child, &status, 0);
  return -1;
}

/* Wait for a child to reject its startup configuration. */
static int wait_child_failure(pid_t child) {
  struct timespec delay = {.tv_sec = 0, .tv_nsec = 100000000};
  int status;

  for (int attempt = 0; attempt < 30; attempt++) {
    pid_t result = waitpid(child, &status, WNOHANG);

    if (result == child)
      return WIFEXITED(status) && WEXITSTATUS(status) != 0 ? 0 : -1;
    if (result < 0)
      return -1;
    nanosleep(&delay, nullptr);
  }
  kill(child, SIGKILL);
  waitpid(child, &status, 0);
  return -1;
}

/* Reserve and return an unused loopback TCP port. */
static int choose_port(void) {
  struct sockaddr_in address = {.sin_family = AF_INET,
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
  socklen_t address_size = sizeof(address);
  int socket_fd = socket(AF_INET, SOCK_STREAM, 0);

  if (socket_fd < 0 ||
      bind(socket_fd, (struct sockaddr *)&address, sizeof(address)) < 0 ||
      getsockname(socket_fd, (struct sockaddr *)&address, &address_size) < 0) {
    if (socket_fd >= 0)
      close(socket_fd);
    return -1;
  }
  close(socket_fd);
  return ntohs(address.sin_port);
}

/* Write the minimal test configuration, leaving all other settings at their
 * configuration_initialize defaults. */
static int write_config(const char *target_path, int port,
                        bool add_color_collision) {
  FILE *target = fopen(target_path, "w");
  bool write_failed;

  if (target == nullptr)
    return -1;
  write_failed =
      fprintf(target,
              "[database]\n"
              "game_database = \"data/stompymux.db\"\n"
              "\n"
              "[server]\n"
              "port = %d\n"
              "\n"
              "[colors]\n"
              "stompy-orange = [255, 112, 0]\n"
              "bright-cyan = [0, 255, 255]\n",
              port) < 0 ||
      (add_color_collision && fputs("red = [1, 2, 3]\n", target) < 0) ||
      fputs("\n"
            "[aliases.flags]\n"
            "AuD = \"AuDiBlE\"\n"
            "\n"
            "[osc8.presets]\n"
            "osc8-demo-button = 'color=white bg=green bold "
            "hover.bg=darkgreen'\n"
            "osc8-demo-danger = 'color=white bg=red bold "
            "hover.bg=darkred disabled.strikethrough'\n",
            target) < 0;
  return fclose(target) == 0 && !write_failed ? 0 : -1;
}

static int remove_database_file(const char *directory, const char *suffix) {
  char path[PATH_MAX];

  return snprintf(path, sizeof(path), "%s/data/stompymux.db%s", directory,
                  suffix) >= (int)sizeof(path) ||
                 (unlink(path) < 0 && errno != ENOENT)
             ? -1
             : 0;
}

static int remove_game_database(const char *directory) {
  return remove_database_file(directory, "") < 0 ||
                 remove_database_file(directory, "-shm") < 0 ||
                 remove_database_file(directory, "-wal") < 0
             ? -1
             : 0;
}

static int send_command(int socket_fd, const char *command);
static int expect_text(int socket_fd, const char *expected);

static int write_nested_lock_fixture(const char *directory) {
  char path[PATH_MAX];

  if (snprintf(path, sizeof(path), "%s/lua/object_logic/nested_lock.lua",
               directory) >= (int)sizeof(path))
    return -1;
  FILE *file = fopen(path, "w");

  if (!file)
    return -1;
  bool failed =
      fputs("return {\n"
            "  locks = {\n"
            "    use = function(ctx)\n"
            "      assert(ctx.descriptor ~= nil, \"nested lock lost "
            "descriptor\")\n"
            "      mux.world.object(ctx.subject):state(\"nested_lock\")"
            ":set(\"inner\", \"discarded\")\n"
            "      error(\"expected nested lock failure\")\n"
            "    end,\n"
            "  },\n"
            "}\n",
            file) < 0;

  return fclose(file) == 0 && !failed ? 0 : -1;
}

static int write_lua_fixture(const char *directory) {
  char path[PATH_MAX];
  FILE *file;

  if (write_nested_lock_fixture(directory) < 0)
    return -1;
  snprintf(path, sizeof(path), "%s/lua/global_logic/styled_text_test.lua",
           directory);
  file = fopen(path, "w");
  if (!file)
    return -1;
  fputs("local safe_thing\n"
        "local stale_thing\n"
        "local stale_dbref\n"
        "local first_startup_count = 0\n"
        "local startup_count = 0\n"
        "local connect_count = 0\n"
        "local startup_order = {}\n",
        file);
  fputs("local btech_api = {\n"
        "  unit = \"armor_status armor_status_ref battle_value "
        "battle_value_ref battle_value2_ref crit_slot crit_slot_ref "
        "crit_status crit_status_ref damage defensive_battle_value_ref "
        "engine_rating engine_rating_ref fasa_base_cost_ref frequencies load "
        "make_pilot_roll offensive_battle_value_ref payload_ref "
        "real_max_speed section_status "
        "set_armor_status set_max_speed set_tons show_crit_status_ref "
        "show_status_ref show_weapon_specs_ref tic_weapons weapon_status "
        "weapon_status_ref\",\n"
        "  map = \"blast_zones elevation emit hex_emit hex_in_blast_zone "
        "hex_line_of_sight id_to_dbref load range set_xy terrain "
        "unit_line_of_sight units update_links\",\n"
        "  parts = \"add_stores categories cost installed installed_ref list "
        "match name remove_stores set_cost stores stores_short type "
        "weapon_stat weight\",\n"
        "  character = \"list set_value threshold value\",\n"
        "  repair = \"damages job_count tech_list tech_list_ref tech_status "
        "tech_time under_repair unit_fixable\",\n"
        "  system = \"design_exists lag set_xcode_value xcode_value "
        "xcode_value_ref zone_units\",\n"
        "}\n"
        "for pkg, names in pairs(btech_api) do\n"
        "  assert(type(btech[pkg]) == \"table\")\n"
        "  local expected = {}\n"
        "  for name in names:gmatch(\"%S+\") do\n"
        "    expected[name] = true\n"
        "    assert(type(btech[pkg][name]) == \"function\")\n"
        "  end\n"
        "  for name in pairs(btech[pkg]) do assert(expected[name]) end\n"
        "end\n"
        "local btech_roots = { character = true, error = true, map = true, "
        "parts = true, repair = true, system = true, unit = true }\n"
        "for name in pairs(btech) do assert(btech_roots[name]) end\n",
        file);
  fputs("assert(type(mux.config.get(\"game_database\")) == \"string\")\n"
        "assert(type(mux.config.get(\"btech_explode_time\")) == \"number\")\n"
        "assert(type(mux.config.get(\"player_name_spaces\")) == \"boolean\")\n"
        "assert(type(mux.config.get(\"btech_allow_cargo_commands\")) == "
        "\"boolean\")\n"
        "assert(type(mux.config.get(\"btech_techtime_multiplier\")) == "
        "\"number\")\n"
        "assert(type(mux.config.get(\"lua_error_reporting\")) == \"string\")\n"
        "local bad_arg_ok, bad_arg = mux.error.pcall(mux.config.get, 1)\n"
        "assert(not bad_arg_ok and bad_arg.code == \"mux.arg.invalid\")\n"
        "local nul_ok, nul_error = mux.error.pcall(mux.config.get, "
        "\"mud_name\\0x\")\n"
        "assert(not nul_ok and nul_error.code == \"mux.arg.invalid\")\n"
        "local missing_ok, missing_error = mux.error.pcall(mux.config.get, "
        "\"MUD_NAME\")\n"
        "assert(not missing_ok and missing_error.code == "
        "\"mux.config.not_found\")\n"
        "local unsupported_ok, unsupported_error = "
        "mux.error.pcall(mux.config.get, \"alias\")\n"
        "assert(not unsupported_ok and unsupported_error.code == "
        "\"mux.config.unsupported\")\n",
        file);
  fputs("return {\n"
        "  events = {\n"
        "    on_server_first_startup = function(ctx)\n"
        "      assert(ctx.scope == \"global\" and ctx.object == nil)\n"
        "      assert(ctx.enactor == 1 and ctx.cause == 1)\n"
        "      for object = 0, 2 do\n"
        "        assert(mux.world.object(object):dbref() == object)\n"
        "      end\n"
        "      first_startup_count = first_startup_count + 1\n"
        "      table.insert(startup_order, \"first\")\n"
        "    end,\n"
        "    on_server_startup = function(ctx)\n"
        "      assert(ctx.scope == \"global\" and ctx.object == nil)\n"
        "      startup_count = startup_count + 1\n"
        "      table.insert(startup_order, \"startup\")\n"
        "    end,\n"
        "    on_player_connect = function(ctx)\n"
        "      assert(ctx.scope == \"global\" and ctx.object == nil)\n"
        "      assert(type(ctx.descriptor) == \"number\")\n"
        "      assert(type(ctx.reconnect) == \"boolean\")\n"
        "      connect_count = connect_count + 1\n"
        "    end,\n"
        "    on_player_disconnect = function(ctx)\n"
        "      assert(ctx.scope == \"global\" and ctx.object == nil)\n"
        "      assert(type(ctx.descriptor) == \"number\")\n"
        "      assert(type(ctx.reason) == \"string\")\n"
        "    end,\n"
        "  },\n"
        "  commands = {\n"
        "    {\n"
        "      pattern = \"^luaevents$\",\n"
        "      handler = function(ctx)\n"
        "        mux.world.pemit(ctx.enactor, \"LuaEvents first=\" ..\n"
        "          first_startup_count .. \" startup=\" .. startup_count ..\n"
        "          \" connect=\" .. connect_count .. \" order=\" ..\n"
        "          table.concat(startup_order, \",\"))\n"
        "        return true\n"
        "      end,\n"
        "    },\n"
        "    {\n"
        "      pattern = \"^luacolor$\",\n"
        "      access = mux.world.access.PUBLIC,\n"
        "      handler = function(ctx)\n"
        "        assert(btech.repair.tech_time(ctx.enactor) == 0)\n"
        "        assert(not pcall(btech.repair.tech_time))\n"
        "        assert(mux.object == nil)\n"
        "        assert(mux.connected_players == nil)\n"
        "        assert(mux.who_summary == nil)\n"
        "        assert(mux.flow_start == nil)\n"
        "        assert(mux.notify == nil)\n"
        "        assert(type(mux.check_db) == \"function\")\n"
        "        assert(mux.world.connected_players == nil)\n"
        "        assert(mux.world.who_summary == nil)\n"
        "        assert(type(mux.world.pemit) == \"function\")\n"
        "        assert(type(mux.world.create_object) == \"function\")\n"
        "        assert(mux.world.create_room == nil)\n"
        "        assert(mux.world.create_thing == nil)\n"
        "        assert(mux.world.create_exit == nil)\n"
        "        assert(type(mux.world.list_objects) == \"function\")\n",
        file);
  fputs("        assert(type(mux.comsys) == \"table\")\n"
        "        assert(type(mux.comsys.channel) == \"function\")\n"
        "        assert(type(mux.comsys.create_channel) == \"function\")\n"
        "        assert(type(mux.comsys.destroy_channel) == \"function\")\n"
        "        assert(type(mux.comsys.list_channels) == \"function\")\n"
        "        assert(mux.comsys.Channel == nil)\n"
        "        assert(tostring(mux.world.access.PUBLIC) == \"PUBLIC\")\n"
        "        assert(tostring(mux.world.access.WIZARD) == \"WIZARD\")\n"
        "        assert(tostring(mux.world.access.GOD) == \"GOD\")\n"
        "        assert(mux.world.access.PUBLIC == mux.world.access.PUBLIC)\n"
        "        local access_ok, access_error = mux.error.pcall(function()\n"
        "          return mux.world.access.NOT_AN_ACCESS\n"
        "        end)\n"
        "        assert(not access_ok and\n"
        "          access_error.code == \"mux.access.invalid\")\n"
        "        assert(mux.world.zone == nil)\n"
        "        assert(mux.world.set_zone == nil)\n"
        "        assert(mux.world.affiliation == nil)\n"
        "        assert(mux.world.set_affiliation == nil)\n"
        "        assert(mux.world.lua_parent == nil)\n"
        "        assert(mux.world.set_lua_parent == nil)\n"
        "        local api_object = mux.world.object(ctx.enactor)\n"
        "        assert(type(api_object.dbref) == \"function\")\n"
        "        assert(type(api_object.type) == \"function\")\n"
        "        assert(type(api_object.name) == \"function\")\n"
        "        assert(type(api_object.set_name) == \"function\")\n"
        "        assert(api_object.attribute == nil)\n"
        "        assert(type(api_object.attributes) == \"function\")\n"
        "        assert(type(api_object.zone) == \"function\")\n"
        "        assert(type(api_object.set_zone) == \"function\")\n"
        "        assert(type(api_object.affiliation) == \"function\")\n"
        "        assert(type(api_object.set_affiliation) == \"function\")\n"
        "        assert(type(api_object.lua_parent) == \"function\")\n"
        "        assert(type(api_object.set_lua_parent) == \"function\")\n"
        "        assert(type(api_object.link_exit) == \"function\")\n"
        "        assert(mux.world.link_exit == nil)\n"
        "        assert(type(mux.world.teleport) == \"function\")\n"
        "        assert(type(mux.world.destroy) == \"function\")\n",
        file);
  fputs(
      "        local channel = mux.comsys.create_channel(\"LuaComsys\")\n"
      "        assert(channel == mux.comsys.channel(\"LuaComsys\"))\n"
      "        assert(tostring(channel) == \"channel(LuaComsys)\")\n"
      "        assert(channel:name() == \"LuaComsys\")\n"
      "        assert(channel:object() == nil)\n"
      "        assert(channel:user_count() == 0)\n"
      "        assert(channel:max_user_count() == 0)\n"
      "        assert(channel:message_count() == 0)\n"
      "        channel:set_object(api_object)\n"
      "        assert(channel:object() == api_object)\n"
      "        channel:set_object(nil)\n"
      "        assert(channel:object() == nil)\n"
      "        local missing_object_ok, missing_object_error =\n"
      "          mux.error.pcall(channel.set_object, channel)\n"
      "        assert(not missing_object_ok and\n"
      "          missing_object_error.code == \"mux.arg.invalid\")\n"
      "        local channel_flags = channel:flags()\n"
      "        local public = mux.comsys.flags.PUBLIC\n"
      "        local loud = mux.comsys.flags.LOUD\n"
      "        local transparent = mux.comsys.flags.TRANSPARENT\n"
      "        assert(tostring(public) == \"PUBLIC\")\n"
      "        assert(public == mux.comsys.flags.PUBLIC)\n"
      "        assert(not channel_flags:has(public))\n"
      "        assert(channel_flags:add(public))\n"
      "        assert(not channel_flags:add(public))\n"
      "        assert(channel_flags:add(loud))\n"
      "        assert(channel_flags:add(transparent))\n"
      "        local listed_flags = channel_flags:list()\n"
      "        assert(#listed_flags == 3 and listed_flags[1] == public)\n"
      "        assert(listed_flags[2] == loud and listed_flags[3] == "
      "transparent)\n"
      "        assert(channel_flags:remove(public))\n"
      "        assert(not channel_flags:remove(public))\n"
      "        local raw_channel_flag_ok, raw_channel_flag_error =\n"
      "          mux.error.pcall(channel_flags.has, channel_flags, "
      "\"PUBLIC\")\n"
      "        assert(not raw_channel_flag_ok and\n"
      "          raw_channel_flag_error.code == \"mux.channel_flag.invalid\")\n"
      "        local unknown_channel_flag_ok, unknown_channel_flag_error =\n"
      "          mux.error.pcall(function()\n"
      "            return mux.comsys.flags.NOT_A_FLAG\n"
      "          end)\n"
      "        assert(not unknown_channel_flag_ok and\n"
      "          unknown_channel_flag_error.code == "
      "\"mux.channel_flag.invalid\")\n"
      "        local mutate_flag_ok, mutate_flag_error =\n"
      "          mux.error.pcall(function() public.value = 1 end)\n"
      "        assert(not mutate_flag_ok and\n"
      "          mutate_flag_error.code == \"mux.channel_flag.invalid\")\n"
      "        assert(#channel:who() == 0 and #channel:who({ all = true }) == "
      "0)\n"
      "        channel:emit(\"LuaComsys headed\")\n"
      "        channel:emit(\"LuaComsys bare\", { no_header = true })\n"
      "        assert(channel:message_count() == 2)\n",
      file);
  fputs(
      "        local alpha = mux.comsys.create_channel(\"A-LuaComsys\")\n"
      "        local zulu = mux.comsys.create_channel(\"z-LuaComsys\")\n"
      "        local channels = mux.comsys.list_channels()\n"
      "        local alpha_index, main_index, zulu_index\n"
      "        for index, candidate in ipairs(channels) do\n"
      "          if candidate:name() == alpha:name() then alpha_index = index "
      "end\n"
      "          if candidate:name() == channel:name() then main_index = index "
      "end\n"
      "          if candidate:name() == zulu:name() then zulu_index = index "
      "end\n"
      "        end\n"
      "        assert(alpha_index < main_index and main_index < zulu_index)\n"
      "        mux.comsys.destroy_channel(alpha)\n"
      "        mux.comsys.destroy_channel(zulu)\n"
      "        local stale = mux.comsys.create_channel(\"LuaComsysStale\")\n"
      "        mux.comsys.destroy_channel(stale)\n"
      "        local stale_ok, stale_error = mux.error.pcall(stale.name, "
      "stale)\n"
      "        assert(not stale_ok and stale_error.code == "
      "\"mux.channel.invalid\")\n"
      "        local replacement = "
      "mux.comsys.create_channel(\"LuaComsysStale\")\n"
      "        assert(stale ~= replacement)\n"
      "        mux.comsys.destroy_channel(replacement)\n"
      "        local duplicate_ok, duplicate_error =\n"
      "          mux.error.pcall(mux.comsys.create_channel, \"LuaComsys\")\n"
      "        assert(not duplicate_ok and duplicate_error.code == "
      "\"mux.channel.invalid\")\n"
      "        local invalid_channel_ok, invalid_channel_error =\n"
      "          mux.error.pcall(mux.comsys.create_channel, \"bad name\")\n"
      "        assert(not invalid_channel_ok and\n"
      "          invalid_channel_error.code == \"mux.arg.invalid\")\n"
      "        local styled_channel = mux.comsys.channel(\"StyledTest\")\n"
      "        local members = styled_channel:who({ all = true })\n"
      "        assert(#members == 1 and members[1].object == api_object)\n"
      "        assert(members[1].listening)\n"
      "        assert(#styled_channel:who() == 1)\n"
      "        styled_channel:boot_player(api_object)\n"
      "        assert(#styled_channel:who({ all = true }) == 0)\n"
      "        mux.comsys.destroy_channel(channel)\n"
      "        assert(type(mux.session.connected_players) == \"function\")\n"
      "        assert(type(mux.session.flow_start) == \"function\")\n"
      "        assert(type(mux.session.who_summary) == \"function\")\n"
      "        assert(mux.markup == nil)\n",
      file);
  fputs(
      "        assert(mux.style == nil)\n"
      "        assert(mux.strip_style == nil)\n"
      "        assert(mux.text_width == nil)\n"
      "        assert(mux.truncate_text == nil)\n"
      "        assert(mux.is_printable_ascii == nil)\n"
      "        assert(mux.text.is_printable_ascii(\"\"))\n"
      "        assert(mux.text.is_printable_ascii(\"A B!~\"))\n"
      "        assert(not mux.text.is_printable_ascii(\"caf\\195\\169\"))\n"
      "        assert(not mux.text.is_printable_ascii(\"a\\0b\"))\n"
      "        assert(not pcall(mux.text.is_printable_ascii, 123))\n"
      "        assert(mux.telnet.environment_has(ctx.descriptor, \"var\", "
      "\"USER\"))\n"
      "        assert(mux.telnet.environment_get(ctx.descriptor, \"var\", "
      "\"USER\") == \"alice\")\n"
      "        assert(not mux.telnet.environment_has(ctx.descriptor, \"var\", "
      "\"EMPTY\"))\n"
      "        assert(mux.telnet.environment_has(ctx.descriptor, \"uservar\", "
      "\"EMPTY\"))\n"
      "        assert(mux.telnet.environment_get(ctx.descriptor, \"uservar\", "
      "\"EMPTY\") == \"\")\n"
      "        assert(mux.telnet.environment_get(ctx.descriptor, \"uservar\", "
      "\"BINARY\") == \"x\\1y\")\n"
      "        assert(mux.telnet.environment_get(ctx.descriptor, \"var\", "
      "\"MISSING\") == nil)\n"
      "        assert(not pcall(mux.telnet.environment_has, ctx.descriptor, "
      "\"bad\", \"USER\"))\n"
      "        local styled = mux.text.style(\"LuaHex\", { foreground = "
      "\"stompy-orange\" })\n"
      "        assert(mux.text.width(styled) == 6)\n"
      "        assert(mux.text.strip_style(styled) == \"LuaHex\")\n"
      "        assert(mux.text.strip_style(mux.text.truncate(styled, 3)) == "
      "\"Lua\")\n"
      "        local object_ok, player = pcall(mux.world.object, ctx.enactor)\n"
      "        assert(object_ok)\n"
      "        assert(player:dbref() == ctx.enactor)\n"
      "        assert(player:type() == mux.world.types.PLAYER)\n"
      "        assert(player.description == nil)\n"
      "        assert(player.inside_description == nil)\n"
      "        assert(player:attributes():get(\"Description\") == nil)\n"
      "        assert(player:attributes():get(\"InternalDescription\") == "
      "nil)\n"
      "        local invalid_ok, invalid_error = "
      "pcall(mux.world.object, 999999)\n"
      "        assert(not invalid_ok)\n"
      "        assert(invalid_error.code == \"mux.object.invalid\")\n"
      "        local checked_ok, checked_player = "
      "mux.error.pcall(mux.world.object, ctx.enactor)\n"
      "        assert(checked_ok)\n"
      "        assert(checked_player:dbref() == ctx.enactor)\n"
      "        local checked_invalid_ok, checked_invalid_error = "
      "mux.error.pcall(mux.world.object, 999999)\n"
      "        assert(not checked_invalid_ok)\n"
      "        assert(checked_invalid_error.code == \"mux.object.invalid\")\n"
      "        mux.world.pemit(ctx.enactor, "
      "mux.text.markup(\"[fg=stompy-orange]LuaMarkup[/]\"))\n"
      "        mux.world.pemit(ctx.enactor, mux.text.markup("
      "\"[link=\\\"https://example.com\\\"]Web[/] \" .."
      " \"[send=\\\"cast fireball\\\"]Cast[/] \" .."
      " \"[prompt=\\\"look\\\"]Edit[/]\"))\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs("    {\n"
        "      pattern = \"^osclinks$\",\n"
        "      handler = function(ctx)\n"
        "        mux.world.pemit(ctx.enactor, mux.text.markup("
        "\"[link=\\\"https://example.com\\\"]Web[/] \" .."
        " \"[send=\\\"cast fireball\\\"]Cast[/] \" .."
        " \"[prompt=\\\"look\\\"]Edit[/]\"))\n"
        "        return true\n"
        "      end,\n"
        "    },\n",
        file);
  fputs(
      "    {\n"
      "      pattern = \"^lualifecycle$\",\n"
      "      handler = function(ctx)\n"
      "        local room = mux.world.create_object({ type = "
      "mux.world.types.ROOM, name = \"Lua Room\" })\n"
      "        local styled_room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"[fg=red]Lua Styled Room[/]\"\n"
      "        })\n"
      "        local thing = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Thing\", location = room\n"
      "        })\n"
      "        local exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"out;o\", location = room, destination = room\n"
      "        })\n"
      "        local link_destination_one = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Link Destination One\"\n"
      "        })\n"
      "        local link_destination_two = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Link Destination Two\"\n"
      "        })\n"
      "        local linked_exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"Lua Persistently Linked Exit\", location = room\n"
      "        })\n"
      "        local unlinked_exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"Lua Persistently Unlinked Exit\",\n"
      "          location = room, destination = link_destination_one\n"
      "        })\n"
      "        linked_exit:link_exit(link_destination_one)\n"
      "        linked_exit:link_exit(link_destination_two)\n"
      "        unlinked_exit:link_exit(nil)\n"
      "        safe_thing = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Safe\", location = ctx.enactor\n"
      "        })\n"
      "        local object_types = mux.world.types\n"
      "        assert(room:type() == object_types.ROOM and\n"
      "          tostring(room:type()) == \"ROOM\" and\n"
      "          room:name() == \"Lua Room\")\n"
      "        assert(styled_room:name() == \"[fg=red]Lua Styled Room[/]\")\n"
      "        assert(thing:type() == object_types.THING and\n"
      "          thing:name() == \"Lua Thing\")\n"
      "        assert(exit:type() == object_types.EXIT and\n"
      "          exit:name() == \"out;o\")\n"
      "        thing:attributes():set(\"Description\", \"Lua description\")\n"
      "        thing:attributes():set(\"InternalDescription\", \"Lua "
      "inside\")\n"
      "        assert(thing:attributes():get(\"Description\") == \"Lua "
      "description\")\n"
      "        assert(thing:attributes():get(\"InternalDescription\") == "
      "\"Lua inside\")\n"
      "        local attributes = thing:attributes()\n"
      "        local entries = attributes:entries()\n"
      "        assert(entries.Description == \"Lua description\")\n"
      "        assert(entries.InternalDescription == \"Lua inside\")\n"
      "        assert(entries.Desc == nil and entries.Idesc == nil)\n"
      "        local old_desc_ok, old_desc_error = mux.error.pcall(\n"
      "          attributes.get, attributes, \"Desc\")\n"
      "        assert(not old_desc_ok and\n"
      "          old_desc_error.code == \"mux.attribute.invalid\")\n"
      "        local old_idesc_ok, old_idesc_error = mux.error.pcall(\n"
      "          attributes.get, attributes, \"Idesc\")\n"
      "        assert(not old_idesc_ok and\n"
      "          old_idesc_error.code == \"mux.attribute.invalid\")\n"
      "        thing:set_name(\"[fg=green]Renamed Lua Thing[/]\")\n"
      "        assert(thing:name() == \"[fg=green]Renamed Lua Thing[/]\")\n"
      "        local missing_name_ok, missing_name_error = mux.error.pcall(\n"
      "          thing.set_name, thing)\n"
      "        assert(not missing_name_ok and missing_name_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local invalid_name_ok, invalid_name_error = mux.error.pcall(\n"
      "          thing.set_name, thing, \"#invalid\")\n"
      "        assert(not invalid_name_ok and invalid_name_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local player = mux.world.object(ctx.enactor)\n"
      "        assert(player:type() == object_types.PLAYER)\n"
      "        local type_mutation_ok, type_mutation_error = mux.error.pcall(\n"
      "          function() object_types.PLAYER = object_types.THING end)\n"
      "        assert(not type_mutation_ok and\n"
      "          type_mutation_error.code == \"mux.arg.invalid\")\n"
      "        local player_name = player:name()\n"
      "        player:set_name(\"LuaRenamedPlayer\")\n"
      "        assert(player:name() == \"LuaRenamedPlayer\")\n"
      "        player:set_name(player_name)\n"
      "        assert(player:name() == player_name)\n",
      file);
  fputs(
      "        local all_contents = room:contents()\n"
      "        assert(#all_contents == 4 and all_contents[1] == thing)\n"
      "        local things = room:contents({\n"
      "          types = { object_types.THING }\n"
      "        })\n"
      "        assert(#things == 1 and things[1] == thing)\n"
      "        assert(#room:contents({\n"
      "          types = { object_types.EXIT }\n"
      "        }) == 3)\n"
      "        assert(#room:contents({ types = {} }) == 0)\n"
      "        assert(#room:contents({ visible_to = ctx.enactor }) == 4)\n"
      "        assert(thing:flags():add(mux.world.flags.DARK))\n"
      "        assert(exit:flags():add(mux.world.flags.DARK))\n"
      "        assert(#room:contents({\n"
      "          types = { object_types.THING }, visible_to = ctx.enactor\n"
      "        }) == 0)\n"
      "        assert(#room:contents({\n"
      "          types = { object_types.EXIT }, visible_to = ctx.enactor\n"
      "        }) == 2)\n"
      "        assert(#room:contents() == 4)\n"
      "        assert(room.contents_visible == nil and room.exits == nil and\n"
      "          room.exits_visible == nil)\n"
      "        local string_type_ok, string_type_error = mux.error.pcall(\n"
      "          room.contents, room, { types = { \"thing\" } })\n"
      "        assert(not string_type_ok and\n"
      "          string_type_error.code == \"mux.arg.invalid\")\n"
      "        local keyed_types_ok, keyed_types_error = mux.error.pcall(\n"
      "          room.contents, room, {\n"
      "            types = { object_types.EXIT, extra = true }\n"
      "          })\n"
      "        assert(not keyed_types_ok and\n"
      "          keyed_types_error.code == \"mux.arg.invalid\")\n"
      "        local unknown_contents_ok, unknown_contents_error =\n"
      "          mux.error.pcall(room.contents, room, { recursive = true })\n"
      "        assert(not unknown_contents_ok and\n"
      "          unknown_contents_error.code == \"mux.arg.invalid\")\n",
      file);
  fputs("        local list_zone = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua List Zone\", location = room\n"
        "        })\n"
        "        local other_list_zone = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Other List Zone\", location = room\n"
        "        })\n"
        "        local empty_list_zone = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Empty List Zone\", location = room\n"
        "        })\n"
        "        thing:set_zone(list_zone)\n"
        "        exit:set_zone(list_zone)\n"
        "        linked_exit:set_zone(other_list_zone)\n"
        "        local listed = mux.world.list_objects()\n"
        "        local listed_nil = mux.world.list_objects(nil)\n"
        "        local listed_empty = mux.world.list_objects({})\n"
        "        assert(#listed == #listed_nil and #listed == #listed_empty)\n"
        "        local found_room = false\n"
        "        for index, object in ipairs(listed) do\n"
        "          assert(object:type() ~= nil)\n"
        "          if object == room then found_room = true end\n"
        "          assert(object == listed_nil[index]\n"
        "            and object == listed_empty[index])\n"
        "          if index > 1 then\n"
        "            assert(listed[index - 1]:dbref() < object:dbref())\n"
        "          end\n"
        "        end\n"
        "        assert(found_room)\n"
        "        local listed_players = mux.world.list_objects({\n"
        "          types = { object_types.PLAYER }\n"
        "        })\n"
        "        local found_enactor = false\n"
        "        for _, player in ipairs(listed_players) do\n"
        "          if player:dbref() == ctx.enactor then found_enactor = true "
        "end\n"
        "        end\n"
        "        assert(found_enactor)\n"
        "        assert(#mux.world.list_objects({\n"
        "          in_zone = empty_list_zone\n"
        "        }) == 0)\n"
        "        local in_zone = mux.world.list_objects({\n"
        "          in_zone = list_zone\n"
        "        })\n"
        "        assert(#in_zone == 2 and in_zone[1] == thing\n"
        "          and in_zone[2] == exit)\n"
        "        local in_zone_dbref = mux.world.list_objects({\n"
        "          in_zone = list_zone:dbref()\n"
        "        })\n"
        "        assert(#in_zone_dbref == 2 and in_zone_dbref[1] == thing\n"
        "          and in_zone_dbref[2] == exit)\n"
        "        local zoned_things = mux.world.list_objects({\n"
        "          types = { object_types.THING }, in_zone = list_zone\n"
        "        })\n"
        "        assert(#zoned_things == 1 and zoned_things[1] == thing)\n",
        file);
  fputs("        local zoned_kinds = mux.world.list_objects({\n"
        "          types = { object_types.THING, object_types.EXIT },\n"
        "          in_zone = list_zone\n"
        "        })\n"
        "        assert(#zoned_kinds == 2 and zoned_kinds[1] == thing\n"
        "          and zoned_kinds[2] == exit)\n"
        "        assert(#mux.world.list_objects({ types = {} }) == 0)\n"
        "        local list_type_ok, list_type_error = mux.error.pcall(\n"
        "          mux.world.list_objects, { types = { \"thing\" } })\n"
        "        assert(not list_type_ok and\n"
        "          list_type_error.code == \"mux.arg.invalid\")\n"
        "        local list_key_ok, list_key_error = mux.error.pcall(\n"
        "          mux.world.list_objects, {\n"
        "            types = { object_types.EXIT, extra = true }\n"
        "          })\n"
        "        assert(not list_key_ok and\n"
        "          list_key_error.code == \"mux.arg.invalid\")\n"
        "        local list_table_ok, list_table_error = mux.error.pcall(\n"
        "          mux.world.list_objects, true)\n"
        "        assert(not list_table_ok and\n"
        "          list_table_error.code == \"mux.arg.invalid\")\n"
        "        local list_unknown_ok, list_unknown_error = mux.error.pcall(\n"
        "          mux.world.list_objects, { recursive = true })\n"
        "        assert(not list_unknown_ok and\n"
        "          list_unknown_error.code == \"mux.arg.invalid\")\n"
        "        local list_zone_ok, list_zone_error = mux.error.pcall(\n"
        "          mux.world.list_objects, { in_zone = 999999 })\n"
        "        assert(not list_zone_ok and\n"
        "          list_zone_error.code == \"mux.object.invalid\")\n"
        "        mux.world.destroy(empty_list_zone)\n"
        "        local going_list_zone_ok, going_list_zone_error =\n"
        "          mux.error.pcall(mux.world.list_objects, {\n"
        "            in_zone = empty_list_zone\n"
        "          })\n"
        "        assert(not going_list_zone_ok and\n"
        "          going_list_zone_error.code == \"mux.object.unavailable\")\n",
        file);
  fputs("        local missing_ok, missing_error = mux.error.pcall(\n"
        "          linked_exit.link_exit, linked_exit)\n"
        "        assert(not missing_ok and missing_error.code == "
        "\"mux.arg.invalid\")\n"
        "        local source_ok, source_error = mux.error.pcall(\n"
        "          room.link_exit, room, nil)\n"
        "        assert(not source_ok and source_error.code == "
        "\"mux.object.invalid\")\n"
        "        local thing_source_ok, thing_source_error = mux.error.pcall(\n"
        "          thing.link_exit, thing, nil)\n"
        "        assert(not thing_source_ok and thing_source_error.code == "
        "\"mux.object.invalid\")\n"
        "        local player_source_ok, player_source_error = "
        "mux.error.pcall(\n"
        "          player.link_exit, player, nil)\n"
        "        assert(not player_source_ok and player_source_error.code == "
        "\"mux.object.invalid\")\n"
        "        local target_ok, target_error = mux.error.pcall(\n"
        "          linked_exit.link_exit, linked_exit, exit)\n"
        "        assert(not target_ok and target_error.code == "
        "\"mux.object.invalid\")\n"
        "        local going_destination = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Going Destination\", location = room\n"
        "        })\n"
        "        mux.world.destroy(going_destination)\n"
        "        local destination_ok, destination_error = mux.error.pcall(\n"
        "          linked_exit.link_exit, linked_exit, going_destination)\n"
        "        assert(not destination_ok and destination_error.code == "
        "\"mux.object.unavailable\")\n"
        "        local going_exit = mux.world.create_object({ type = "
        "mux.world.types.EXIT,\n"
        "          name = \"Lua Going Exit\", location = room\n"
        "        })\n"
        "        mux.world.destroy(going_exit)\n"
        "        local going_ok, going_error = mux.error.pcall(\n"
        "          going_exit.link_exit, going_exit, nil)\n"
        "        assert(not going_ok and going_error.code == "
        "\"mux.object.unavailable\")\n",
        file);
  fputs(
      "        local count_before_invalid_create =\n"
      "          #mux.world.list_objects()\n"
      "        local invalid_ok, invalid_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = mux.world.types.ROOM,\n"
      "            name = \"Bad\", location = room })\n"
      "        assert(not invalid_ok and invalid_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local thing_field_ok, thing_field_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = mux.world.types.THING,\n"
      "            name = \"Bad Thing Field\", location = room,\n"
      "            destination = room })\n"
      "        assert(not thing_field_ok and thing_field_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local exit_field_ok, exit_field_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = mux.world.types.EXIT,\n"
      "            name = \"Bad Exit Field\", location = room, home = room })\n"
      "        assert(not exit_field_ok and exit_field_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local zone_kind_ok, zone_kind_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = mux.world.types.ROOM,\n"
      "            name = \"Bad Zone Kind\", zone = exit })\n"
      "        assert(not zone_kind_ok and zone_kind_error.code == "
      "\"mux.object.invalid\")\n"
      "        local missing_type_ok, missing_type_error = mux.error.pcall(\n"
      "          mux.world.create_object, { name = \"Bad Missing Type\" })\n"
      "        assert(not missing_type_ok and missing_type_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local string_type_ok, string_type_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = \"ROOM\",\n"
      "            name = \"Bad String Type\" })\n"
      "        assert(not string_type_ok and string_type_error.code == "
      "\"mux.arg.invalid\")\n"
      "        local player_type_ok, player_type_error = mux.error.pcall(\n"
      "          mux.world.create_object, { type = mux.world.types.PLAYER,\n"
      "            name = \"Bad Player Type\" })\n"
      "        assert(not player_type_ok and player_type_error.code == "
      "\"mux.arg.invalid\")\n"
      "        assert(#mux.world.list_objects() ==\n"
      "          count_before_invalid_create)\n"
      "        mux.world.destroy(list_zone)\n"
      "        mux.world.destroy(other_list_zone)\n"
      "        mux.world.destroy(thing)\n"
      "        stale_thing = thing\n"
      "        stale_dbref = thing:dbref()\n"
      "        mux.world.destroy(styled_room)\n"
      "        assert(thing:type() == mux.world.types.THING)\n"
      "        local destroy_ok, destroy_error = mux.error.pcall(\n"
      "          mux.world.destroy, thing)\n"
      "        assert(not destroy_ok and destroy_error.code == "
      "\"mux.object.unavailable\")\n"
      "        local core_ok, core_error = mux.error.pcall(\n"
      "          mux.world.destroy, ctx.enactor)\n"
      "        assert(not core_ok and core_error.code == "
      "\"mux.object.unavailable\")\n"
      "        mux.world.pemit(ctx.enactor, \"LuaLifecycle room thing exit "
      "destroyed\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs("    {\n"
        "      pattern = \"^luadbck$\",\n"
        "      handler = function(ctx)\n"
        "        mux.check_db()\n"
        "        mux.world.pemit(ctx.enactor, \"LuaDbck complete\")\n"
        "        return true\n"
        "      end,\n"
        "    },\n",
        file);
  fputs(
      "    {\n"
      "      pattern = \"^luasafedestroy$\",\n"
      "      handler = function(ctx)\n"
      "        local safe_ok, safe_error = mux.error.pcall(\n"
      "          mux.world.destroy, safe_thing)\n"
      "        assert(not safe_ok and safe_error.code == "
      "\"mux.object.unavailable\")\n"
      "        mux.world.destroy(safe_thing, { override = true })\n"
      "        mux.world.pemit(ctx.enactor, \"LuaSafe protected overridden\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^luateleport$\",\n"
      "      handler = function(ctx)\n"
      "        local source = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Teleport Source\"\n"
      "        })\n"
      "        local destination = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Teleport Destination\"\n"
      "        })\n"
      "        local thing = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistently Teleported Thing\",\n"
      "          location = source\n"
      "        })\n"
      "        assert(#source:contents() == 1)\n"
      "        local result = mux.world.teleport({\n"
      "          object = thing, destination = destination\n"
      "        })\n"
      "        assert(result == nil and #source:contents() == 0)\n"
      "        assert(#destination:contents() == 1\n"
      "          and destination:contents()[1] == thing)\n"
      "        local unknown_ok, unknown_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = thing,\n"
      "            destination = source, quiet = true })\n"
      "        assert(not unknown_ok\n"
      "          and unknown_error.code == \"mux.arg.invalid\")\n"
      "        local missing_ok, missing_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = thing })\n"
      "        assert(not missing_ok\n"
      "          and missing_error.code == \"mux.arg.invalid\")\n"
      "        local room_ok, room_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = source,\n"
      "            destination = destination })\n"
      "        assert(not room_ok\n"
      "          and room_error.code == \"mux.object.invalid\")\n"
      "        local exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"Lua Teleport Exit\", location = source\n"
      "        })\n"
      "        local exit_ok, exit_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = thing, destination = exit })\n"
      "        assert(not exit_ok\n"
      "          and exit_error.code == \"mux.object.invalid\")\n"
      "        local container = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Teleport Container\", location = source\n"
      "        })\n"
      "        local child = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Teleport Child\", location = container\n"
      "        })\n"
      "        local cycle_ok, cycle_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = container,\n"
      "            destination = child })\n"
      "        assert(not cycle_ok\n"
      "          and cycle_error.code == \"mux.object.invalid\")\n"
      "        local going_destination = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Teleport Going Destination\", location = source\n"
      "        })\n"
      "        mux.world.destroy(going_destination)\n"
      "        local destination_ok, destination_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = thing,\n"
      "            destination = going_destination })\n"
      "        assert(not destination_ok\n"
      "          and destination_error.code == \"mux.object.unavailable\")\n"
      "        local going_object = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Teleport Going Object\", location = source\n"
      "        })\n"
      "        mux.world.destroy(going_object)\n"
      "        local object_ok, object_error = mux.error.pcall(\n"
      "          mux.world.teleport, { object = going_object,\n"
      "            destination = destination })\n"
      "        assert(not object_ok\n"
      "          and object_error.code == \"mux.object.unavailable\")\n"
      "        mux.world.pemit(ctx.enactor, \"LuaTeleport moved\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^luazone$\",\n"
      "      handler = function(ctx)\n"
      "        local room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Zone Room\"\n"
      "        })\n"
      "        local room_zone = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Room Zone\"\n"
      "        })\n"
      "        local thing_zone = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistent Zone\", location = room\n"
      "        })\n"
      "        assert(room:zone() == nil)\n"
      "        local created_zoned_room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Created Zoned Room\", zone = room_zone\n"
      "        })\n"
      "        local created_thing_zoned_room = mux.world.create_object({ type "
      "= mux.world.types.ROOM,\n"
      "          name = \"Lua Created Thing-Zoned Room\", zone = thing_zone\n"
      "        })\n"
      "        local created_nil_zone_room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Created Nil-Zone Room\", zone = nil\n"
      "        })\n"
      "        local created_zoned_thing = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Created Zoned Thing\", location = room,\n"
      "          zone = room_zone:dbref()\n"
      "        })\n"
      "        local created_zoned_exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"Lua Created Zoned Exit\", location = room,\n"
      "          zone = room_zone\n"
      "        })\n"
      "        assert(created_zoned_room:zone() == room_zone)\n"
      "        assert(created_thing_zoned_room:zone() == thing_zone)\n"
      "        assert(created_nil_zone_room:zone() == nil)\n"
      "        assert(created_zoned_thing:zone() == room_zone)\n"
      "        assert(created_zoned_exit:zone() == room_zone)\n"
      "        local created_in_zone = mux.world.list_objects({\n"
      "          in_zone = room_zone\n"
      "        })\n"
      "        assert(#created_in_zone == 3\n"
      "          and created_in_zone[1] == created_zoned_room\n"
      "          and created_in_zone[2] == created_zoned_thing\n"
      "          and created_in_zone[3] == created_zoned_exit)\n"
      "        local target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistently Zoned Thing\", location = room\n"
      "        })\n"
      "        target:set_zone(thing_zone)\n"
      "        assert(target:zone() == thing_zone)\n"
      "        assert(mux.world.object(target:dbref()):zone() == thing_zone)\n"
      "        room:set_zone(room_zone)\n"
      "        assert(room:zone() == room_zone)\n"
      "        room:set_zone(nil)\n"
      "        assert(room:zone() == nil)\n"
      "        local missing_ok, missing_error = mux.error.pcall(\n"
      "          target.set_zone, target)\n"
      "        assert(not missing_ok\n"
      "          and missing_error.code == \"mux.arg.invalid\")\n",
      file);
  fputs("        target:set_zone(room_zone)\n"
        "        assert(target:zone() == room_zone)\n"
        "        target:set_zone(thing_zone)\n"
        "        local exit = mux.world.create_object({ type = "
        "mux.world.types.EXIT,\n"
        "          name = \"Lua Invalid Zone Exit\", location = room\n"
        "        })\n"
        "        local exit_ok, exit_error = mux.error.pcall(\n"
        "          target.set_zone, target, exit)\n"
        "        assert(not exit_ok\n"
        "          and exit_error.code == \"mux.object.invalid\")\n"
        "        local object_count_before_invalid_zone =\n"
        "          #mux.world.list_objects()\n"
        "        local create_dbref_ok, create_dbref_error = mux.error.pcall(\n"
        "          mux.world.create_object, { type = mux.world.types.ROOM, "
        "name = \"Bad Zone Dbref\",\n"
        "            zone = 999999 })\n"
        "        assert(not create_dbref_ok\n"
        "          and create_dbref_error.code == \"mux.object.invalid\")\n"
        "        local create_kind_ok, create_kind_error = mux.error.pcall(\n"
        "          mux.world.create_object, { type = mux.world.types.ROOM, "
        "name = \"Bad Zone Kind\",\n"
        "            zone = exit })\n"
        "        assert(not create_kind_ok\n"
        "          and create_kind_error.code == \"mux.object.invalid\")\n"
        "        assert(#mux.world.list_objects() ==\n"
        "          object_count_before_invalid_zone)\n"
        "        local going_zone = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Going Zone\", location = room\n"
        "        })\n"
        "        mux.world.destroy(going_zone)\n"
        "        local zone_ok, zone_error = mux.error.pcall(\n"
        "          target.set_zone, target, going_zone)\n"
        "        assert(not zone_ok\n"
        "          and zone_error.code == \"mux.object.unavailable\")\n"
        "        local object_count_before_going_zone =\n"
        "          #mux.world.list_objects()\n"
        "        local create_going_ok, create_going_error = mux.error.pcall(\n"
        "          mux.world.create_object, { type = mux.world.types.THING, "
        "name = \"Bad Going Zone\",\n"
        "            location = room, zone = going_zone })\n"
        "        assert(not create_going_ok\n"
        "          and create_going_error.code == \"mux.object.unavailable\")\n"
        "        assert(#mux.world.list_objects() ==\n"
        "          object_count_before_going_zone)\n"
        "        local going_target = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Going Zoned Target\", location = room\n"
        "        })\n"
        "        mux.world.destroy(going_target)\n"
        "        local target_ok, target_error = mux.error.pcall(\n"
        "          going_target.set_zone, going_target, thing_zone)\n"
        "        assert(not target_ok\n"
        "          and target_error.code == \"mux.object.unavailable\")\n"
        "        local cleanup_target = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Cleared Zone Target\", location = room\n"
        "        })\n"
        "        local cleanup_zone = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "          name = \"Lua Destroyed Zone\", location = room\n"
        "        })\n"
        "        cleanup_target:set_zone(cleanup_zone)\n"
        "        mux.world.destroy(cleanup_zone)\n"
        "        assert(cleanup_target:zone() == nil)\n"
        "        mux.world.object(ctx.enactor):set_zone(thing_zone)\n"
        "        mux.world.pemit(ctx.enactor, \"LuaZone assigned\")\n"
        "        return true\n"
        "      end,\n"
        "    },\n",
        file);
  fputs("    {\n"
        "      pattern = \"^luastale$\",\n"
        "      handler = function(ctx)\n"
        "        local still_valid = mux.error.pcall(\n"
        "          stale_thing.dbref, stale_thing)\n"
        "        local replacements = {}\n"
        "        if still_valid then\n"
        "          for _ = 1, 256 do\n"
        "            local replacement = mux.world.create_object({ type = "
        "mux.world.types.THING,\n"
        "              name = \"Lua Stale Replacement\",\n"
        "              location = ctx.enactor\n"
        "            })\n"
        "            replacements[#replacements + 1] = replacement\n"
        "            if replacement:dbref() == stale_dbref then break end\n"
        "          end\n"
        "        end\n"
        "        local dbref_ok, dbref_error = mux.error.pcall(\n"
        "          stale_thing.dbref, stale_thing)\n"
        "        assert(not dbref_ok and\n"
        "          dbref_error.code == \"mux.object.invalid\")\n"
        "        local type_ok, type_error = mux.error.pcall(\n"
        "          stale_thing.type, stale_thing)\n"
        "        assert(not type_ok and\n"
        "          type_error.code == \"mux.object.invalid\")\n"
        "        for _, replacement in ipairs(replacements) do\n"
        "          mux.world.destroy(replacement)\n"
        "        end\n"
        "        mux.world.pemit(ctx.enactor, \"LuaStale rejected\")\n"
        "        return true\n"
        "      end,\n"
        "    },\n",
        file);
  fputs("    {\n"
        "      pattern = \"^luazonereloaded$\",\n"
        "      handler = function(ctx)\n"
        "        local zone = mux.world.object(ctx.enactor):zone()\n"
        "        assert(zone ~= nil\n"
        "          and zone:name() == \"Lua Persistent Zone\")\n"
        "        mux.world.pemit(ctx.enactor, \"LuaZone reloaded\")\n"
        "        return true\n"
        "      end,\n"
        "    },\n",
        file);
  fputs(
      "    {\n"
      "      pattern = \"^luaaffiliation$\",\n"
      "      handler = function(ctx)\n"
      "        local room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Affiliation Room\"\n"
      "        })\n"
      "        local affiliate = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistent Affiliation\", location = room\n"
      "        })\n"
      "        local target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistently Affiliated Thing\",\n"
      "          location = ctx.enactor\n"
      "        })\n"
      "        local exit = mux.world.create_object({ type = "
      "mux.world.types.EXIT,\n"
      "          name = \"Lua Affiliation Exit\", location = room\n"
      "        })\n"
      "        local player = mux.world.object(ctx.enactor)\n"
      "        assert(target:affiliation() == nil)\n"
      "        target:set_affiliation(room)\n"
      "        assert(target:affiliation() == room)\n"
      "        target:set_affiliation(exit)\n"
      "        assert(mux.world.object(target:dbref()):affiliation() == exit)\n"
      "        target:set_affiliation(player)\n"
      "        assert(target:affiliation() == player)\n"
      "        target:set_affiliation(target)\n"
      "        assert(target:affiliation() == target)\n"
      "        target:set_affiliation(nil)\n"
      "        assert(target:affiliation() == nil)\n"
      "        local missing_ok, missing_error = mux.error.pcall(\n"
      "          target.set_affiliation, target)\n"
      "        assert(not missing_ok\n"
      "          and missing_error.code == \"mux.arg.invalid\")\n"
      "        local going_affiliate = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Going Affiliation\", location = room\n"
      "        })\n"
      "        mux.world.destroy(going_affiliate)\n"
      "        local affiliation_ok, affiliation_error = mux.error.pcall(\n"
      "          target.set_affiliation, target, going_affiliate)\n"
      "        assert(not affiliation_ok\n"
      "          and affiliation_error.code == \"mux.object.unavailable\")\n"
      "        local going_target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Going Affiliation Target\", location = room\n"
      "        })\n"
      "        mux.world.destroy(going_target)\n"
      "        local target_ok, target_error = mux.error.pcall(\n"
      "          going_target.set_affiliation, going_target, affiliate)\n"
      "        assert(not target_ok\n"
      "          and target_error.code == \"mux.object.unavailable\")\n"
      "        local cleanup_target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Cleared Affiliation Target\", location = room\n"
      "        })\n"
      "        local cleanup_affiliate = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Destroyed Affiliation\", location = room\n"
      "        })\n"
      "        cleanup_target:set_affiliation(cleanup_affiliate)\n"
      "        mux.world.destroy(cleanup_affiliate)\n"
      "        assert(cleanup_target:affiliation() == nil)\n"
      "        target:set_affiliation(affiliate)\n"
      "        player:set_affiliation(affiliate)\n"
      "        mux.world.pemit(ctx.enactor, \"LuaAffiliation assigned\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^luaaffiliationreloaded$\",\n"
      "      handler = function(ctx)\n"
      "        local affiliate = mux.world.object(ctx.enactor):affiliation()\n"
      "        assert(affiliate ~= nil\n"
      "          and affiliate:name() == \"Lua Persistent Affiliation\")\n"
      "        mux.world.pemit(ctx.enactor, \"LuaAffiliation reloaded\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^lualuaparent$\",\n"
      "      handler = function(ctx)\n"
      "        local room = mux.world.create_object({ type = "
      "mux.world.types.ROOM,\n"
      "          name = \"Lua Parent Room\"\n"
      "        })\n"
      "        local target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistently Parented Thing\", location = room\n"
      "        })\n"
      "        local cleared = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Persistently Unparented Thing\", location = "
      "room\n"
      "        })\n"
      "        assert(target:lua_parent() == \"default_thing.lua\")\n"
      "        local result = target:set_lua_parent("
      "\"example.lua\")\n"
      "        assert(result == nil)\n"
      "        assert(target:lua_parent() == \"example.lua\")\n"
      "        assert(mux.world.object(target:dbref()):lua_parent() == "
      "\"example.lua\")\n"
      "        cleared:set_lua_parent(nil)\n"
      "        assert(cleared:lua_parent() == nil)\n"
      "        local missing_ok, missing_error = mux.error.pcall(\n"
      "          target.set_lua_parent, target)\n"
      "        assert(not missing_ok\n"
      "          and missing_error.code == \"mux.arg.invalid\")\n"
      "        local type_ok, type_error = mux.error.pcall(\n"
      "          target.set_lua_parent, target, 7)\n"
      "        assert(not type_ok\n"
      "          and type_error.code == \"mux.arg.invalid\")\n"
      "        local nul_ok, nul_error = mux.error.pcall(\n"
      "          target.set_lua_parent, target, \"example.lua\\0ignored\")\n"
      "        assert(not nul_ok\n"
      "          and nul_error.code == \"mux.arg.invalid\")\n"
      "        local prefix_ok, prefix_error = mux.error.pcall(\n"
      "          target.set_lua_parent, target,\n"
      "            \"object_logic/example.lua\")\n"
      "        assert(not prefix_ok\n"
      "          and prefix_error.code == \"mux.module.invalid\")\n"
      "        local absent_ok, absent_error = mux.error.pcall(\n"
      "          target.set_lua_parent, target, \"absent.lua\")\n"
      "        assert(not absent_ok\n"
      "          and absent_error.code == \"mux.module.invalid\")\n"
      "        local going = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Going Parent Target\", location = room\n"
      "        })\n"
      "        mux.world.destroy(going)\n"
      "        local going_ok, going_error = mux.error.pcall(\n"
      "          going.set_lua_parent, going, \"example.lua\")\n"
      "        assert(not going_ok\n"
      "          and going_error.code == \"mux.object.unavailable\")\n"
      "        mux.world.pemit(ctx.enactor, \"LuaParent assigned\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^luastate$\",\n"
      "      handler = function(ctx)\n"
      "        local object = mux.world.object(ctx.enactor)\n"
      "        local state = object:state(\"integration\")\n"
      "        local balance = state:get(\"balance\", 0) + 1\n"
      "        state:set_many({ balance = balance, enabled = true,\n"
      "          memo = \"\", rate = 1.25 })\n"
      "        object:state(\"audit\"):set(\"last_balance\", balance)\n"
      "        local examined = mux.world.object(4)\n"
      "        examined:state(\"integration\"):set_many({\n"
      "          balance = balance, enabled = true, memo = \"\", rate = 1.25\n"
      "        })\n"
      "        examined:state(\"audit\"):set(\"last_balance\", balance)\n"
      "        assert(state:has(\"memo\") and state:get(\"memo\") == \"\")\n"
      "        assert(#state:keys() == 4 and #state:entries() == 4)\n"
      "        local values = state:get_many({ \"balance\", \"enabled\" })\n"
      "        assert(values.balance == balance and values.enabled)\n"
      "        mux.world.pemit(ctx.enactor, \"LuaState \" .. balance)\n"
      "        return true\n"
      "      end,\n"
      "    },\n",
      file);
  fputs(
      "    {\n"
      "      pattern = \"^lualocknested$\",\n"
      "      handler = function(ctx)\n"
      "        local actor = mux.world.object(ctx.enactor)\n"
      "        local target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Nested Lock Target\", location = actor\n"
      "        })\n"
      "        target:set_lua_parent(\"nested_lock.lua\")\n"
      "        local state = actor:state(\"nested_lock\")\n"
      "        state:set(\"outer\", \"preserved\")\n"
      "        local passes = mux.world.lock_passes({\n"
      "          object = target, enactor = actor, cause = actor,\n"
      "          subject = actor, lock = mux.world.locks.USE\n"
      "        })\n"
      "        assert(not passes and not state:has(\"inner\"))\n"
      "        state:set(\"after\", \"committed\")\n"
      "        mux.world.pemit(ctx.enactor, \"LuaLock nested isolated\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n"
      "    {\n"
      "      pattern = \"^lualockverify$\",\n"
      "      handler = function(ctx)\n"
      "        local state = mux.world.object(ctx.enactor):state("
      "\"nested_lock\")\n"
      "        assert(state:get(\"outer\") == \"preserved\")\n"
      "        assert(state:get(\"after\") == \"committed\")\n"
      "        assert(not state:has(\"inner\"))\n"
      "        mux.world.pemit(ctx.enactor, \"LuaLock state verified\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n"
      "    {\n"
      "      pattern = \"^luaflags$\",\n"
      "      handler = function(ctx)\n"
      "        local room = mux.world.create_object({ type = "
      "mux.world.types.ROOM, name = \"Lua Flags Room\" "
      "})\n"
      "        local target = mux.world.create_object({ type = "
      "mux.world.types.THING,\n"
      "          name = \"Lua Flags Target\", location = room\n"
      "        })\n"
      "        local flags = target:flags()\n"
      "        local powers = target:powers()\n"
      "        local safe = mux.world.flags.SAFE\n"
      "        local transparent = mux.world.flags.TRANSPARENT\n"
      "        local idle = mux.world.powers.IDLE\n"
      "        assert(safe == mux.world.flags.SAFE and tostring(safe) == "
      "\"SAFE\")\n"
      "        assert(idle == mux.world.powers.IDLE and tostring(idle) == "
      "\"IDLE\")\n"
      "        assert(tostring(flags):match(\"^flags%(%#%d+%)$\"))\n"
      "        assert(tostring(powers):match(\"^powers%(%#%d+%)$\"))\n"
      "        assert(not flags:has(safe) and flags:add(safe))\n"
      "        assert(flags:has(safe) and not flags:add(safe))\n"
      "        assert(flags:add(transparent))\n"
      "        local listed_flags = flags:list()\n"
      "        local safe_index, transparent_index\n"
      "        for index, flag in ipairs(listed_flags) do\n"
      "          if flag == safe then safe_index = index end\n"
      "          if flag == transparent then transparent_index = index end\n"
      "        end\n"
      "        assert(safe_index and transparent_index\n"
      "          and safe_index < transparent_index)\n"
      "        assert(flags:remove(safe) and not flags:remove(safe))\n"
      "        assert(flags:remove(transparent))\n"
      "        assert(not flags:add(mux.world.flags.XCODE))\n"
      "        assert(not flags:has(mux.world.flags.XCODE))\n"
      "        assert(not powers:has(idle) and powers:add(idle))\n"
      "        assert(powers:has(idle) and not powers:add(idle))\n"
      "        local listed_powers = powers:list()\n"
      "        assert(#listed_powers == 1 and listed_powers[1] == idle)\n"
      "        assert(powers:remove(idle) and not powers:remove(idle))\n"
      "        local raw_ok, raw_error = mux.error.pcall(flags.has, flags, "
      "\"SAFE\")\n"
      "        assert(not raw_ok and raw_error.code == \"mux.flag.invalid\")\n"
      "        local kind_ok, kind_error = mux.error.pcall(\n"
      "          flags.has, flags, idle)\n"
      "        assert(not kind_ok and kind_error.code == "
      "\"mux.flag.invalid\")\n"
      "        local name_ok, name_error = mux.error.pcall(function()\n"
      "          return mux.world.flags.NOT_A_FLAG\n"
      "        end)\n"
      "        assert(not name_ok and name_error.code == "
      "\"mux.flag.invalid\")\n"
      "        local write_ok, write_error = mux.error.pcall(function()\n"
      "          mux.world.powers.IDLE = idle\n"
      "        end)\n"
      "        assert(not write_ok and write_error.code == "
      "\"mux.power.invalid\")\n"
      "        mux.world.destroy(target)\n"
      "        mux.world.destroy(room)\n"
      "        mux.world.pemit(ctx.enactor, \"LuaFlags constants changed\")\n"
      "        return true\n"
      "      end,\n"
      "    },\n"
      "    {\n"
      "      pattern = \"^luafail$\",\n"
      "      handler = function(ctx)\n"
      "        mux.world.object(ctx.enactor):state(\"integration\")"
      ":set(\"balance\", 99)\n"
      "        error(\"expected rollback\")\n"
      "      end,\n"
      "    },\n"
      "  },\n"
      "}\n",
      file);
  return fclose(file) == 0 ? 0 : -1;
}

static int exercise_invalid_lock_keys(int socket_fd, const char *directory) {
  static const char *const INVALID_KEYS[] = {"default", "speech", "look",
                                             "contact"};
  char path[PATH_MAX];

  if (snprintf(path, sizeof(path), "%s/lua/object_logic/invalid_lock.lua",
               directory) >= (int)sizeof(path))
    return -1;
  for (size_t index = 0; index < sizeof(INVALID_KEYS) / sizeof(*INVALID_KEYS);
       index++) {
    const char *key = *(const char *const *)checked_storage_at_const(
        INVALID_KEYS, sizeof(INVALID_KEYS) / sizeof(*INVALID_KEYS),
        sizeof(*INVALID_KEYS), index);
    FILE *file = fopen(path, "w");

    if (!file)
      return -1;
    bool failed = fprintf(file,
                          "return { locks = { %s = function() return true "
                          "end } }\n",
                          key) < 0;
    if (fclose(file) != 0 || failed ||
        send_command(socket_fd, "@lua/check\r\n") < 0) {
      (void)unlink(path);
      return -1;
    }
    char expected[128];

    if (snprintf(expected, sizeof(expected), "unknown lock key '%s'", key) >=
            (int)sizeof(expected) ||
        expect_text(socket_fd, expected) < 0) {
      (void)unlink(path);
      return -1;
    }
    if (unlink(path) < 0)
      return -1;
  }
  return 0;
}

/* Connect one client to port once the child server begins listening. */
static int connect_when_ready(int port) {
  struct sockaddr_in address = {.sin_family = AF_INET,
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK),
                                .sin_port = htons((uint16_t)port)};
  struct timespec delay = {.tv_sec = 0, .tv_nsec = 100000000};

  for (int attempt = 0; attempt < 50; attempt++) {
    int socket_fd = socket(AF_INET, SOCK_STREAM, 0);

    if (socket_fd >= 0 &&
        connect(socket_fd, (struct sockaddr *)&address, sizeof(address)) == 0)
      return socket_fd;
    if (socket_fd >= 0)
      close(socket_fd);
    nanosleep(&delay, nullptr);
  }
  return -1;
}

static int send_bytes(int socket_fd, const unsigned char *bytes, size_t size) {
  size_t sent = 0;

  while (sent < size) {
    ssize_t result = write(socket_fd, constant_buffer_suffix(bytes, size, sent),
                           size - sent);

    if (result < 0 && errno == EINTR)
      continue;
    if (result <= 0) {
      fprintf(stderr, "socket write failed: %s\n",
              result < 0 ? strerror(errno) : "connection closed");
      return -1;
    }
    sent += (size_t)result;
  }
  return 0;
}

static int64_t monotonic_milliseconds(void) {
  struct timespec now;

  if (clock_gettime(CLOCK_MONOTONIC, &now) < 0)
    return -1;
  return ((int64_t)now.tv_sec * 1000) + (now.tv_nsec / 1000000);
}

static void report_received_bytes(const TelnetTestClient *client) {
  const size_t DISPLAY_SIZE =
      client->received_size < 128 ? client->received_size : 128;

  fprintf(stderr, "received %zu byte(s):", client->received_size);
  for (size_t index = 0; index < DISPLAY_SIZE; index++) {
    const unsigned char *byte = checked_storage_at_const(
        client->received, sizeof(client->received), sizeof(*byte), index);
    fprintf(stderr, " %02x", *byte);
  }
  if (DISPLAY_SIZE < client->received_size)
    fprintf(stderr, " ...");
  fputc('\n', stderr);
}

static int wait_for_patterns(TelnetTestClient *client, const char *phase,
                             const void *first, size_t first_size,
                             const void *second, size_t second_size) {
  const int64_t START = monotonic_milliseconds();
  const int64_t DEADLINE = START < 0 ? -1 : START + TEST_IO_TIMEOUT_MS;

  while (DEADLINE >= 0) {
    const bool FOUND_FIRST = memmem(client->received, client->received_size,
                                    first, first_size) != nullptr;
    const bool FOUND_SECOND =
        second == nullptr || memmem(client->received, client->received_size,
                                    second, second_size) != nullptr;
    if (FOUND_FIRST && FOUND_SECOND) {
      client->received_size = 0;
      return 0;
    }

    const int64_t NOW = monotonic_milliseconds();
    if (NOW < 0 || NOW >= DEADLINE)
      break;
    struct pollfd readable = {.fd = client->socket_fd, .events = POLLIN};
    int poll_result = poll(&readable, 1, (int)(DEADLINE - NOW));
    if (poll_result < 0 && errno == EINTR)
      continue;
    if (poll_result < 0) {
      fprintf(stderr, "%s poll failed: %s\n", phase, strerror(errno));
      report_received_bytes(client);
      return -1;
    }
    if (poll_result == 0)
      break;
    if (readable.revents & (POLLERR | POLLNVAL)) {
      fprintf(stderr, "%s poll failed with revents 0x%x\n", phase,
              readable.revents);
      report_received_bytes(client);
      return -1;
    }
    if (client->received_size == sizeof(client->received)) {
      fprintf(stderr, "%s receive buffer exhausted\n", phase);
      report_received_bytes(client);
      return -1;
    }
    ssize_t size =
        read(client->socket_fd,
             buffer_suffix(client->received, sizeof(client->received),
                           client->received_size),
             sizeof(client->received) - client->received_size);
    if (size < 0 && errno == EINTR)
      continue;
    if (size <= 0) {
      fprintf(stderr, "%s read failed: %s\n", phase,
              size < 0 ? strerror(errno) : "connection closed");
      report_received_bytes(client);
      return -1;
    }
    client->received_size += (size_t)size;
  }

  fprintf(stderr, "%s timed out after %d ms\n", phase, TEST_IO_TIMEOUT_MS);
  report_received_bytes(client);
  return -1;
}

static int wait_for_pattern(TelnetTestClient *client, const char *phase,
                            const void *pattern, size_t pattern_size) {
  return wait_for_patterns(client, phase, pattern, pattern_size, nullptr, 0);
}

static int expect_ttype_request(TelnetTestClient *client) {
  static const unsigned char request[] = {
      TELNET_IAC,        TELNET_SB,  TELNET_TTYPE,
      TELNET_TTYPE_SEND, TELNET_IAC, TELNET_SE,
  };
  return wait_for_pattern(client, "TTYPE request", request, sizeof(request));
}

static int negotiate_utf8(TelnetTestClient *client) {
  static const unsigned char enable[] = {TELNET_IAC, TELNET_DO, TELNET_CHARSET};
  static const unsigned char request[] = {TELNET_IAC, TELNET_SB, 42,  1,   ';',
                                          'U',        'T',       'F', '-', '8',
                                          TELNET_IAC, TELNET_SE};
  static const unsigned char accepted[] = {
      TELNET_IAC, TELNET_SB, 42,  2,          'U',      'T',
      'F',        '-',       '8', TELNET_IAC, TELNET_SE};
  if (send_bytes(client->socket_fd, enable, sizeof(enable)) < 0 ||
      wait_for_pattern(client, "UTF-8 CHARSET request", request,
                       sizeof(request)) < 0)
    return -1;
  return send_bytes(client->socket_fd, accepted, sizeof(accepted));
}

static int negotiate_new_environ(TelnetTestClient *client) {
  static const unsigned char enable[] = {TELNET_IAC, TELNET_WILL,
                                         TELNET_NEW_ENVIRON};
  static const unsigned char request[] = {
      TELNET_IAC,          TELNET_SB,  TELNET_NEW_ENVIRON,
      TELNET_ENVIRON_SEND, TELNET_IAC, TELNET_SE};
  static const unsigned char response[] = {
      TELNET_IAC,
      TELNET_SB,
      TELNET_NEW_ENVIRON,
      TELNET_ENVIRON_IS,
      TELNET_ENVIRON_VAR,
      'U',
      'S',
      'E',
      'R',
      TELNET_ENVIRON_VALUE,
      'a',
      'l',
      'i',
      'c',
      'e',
      TELNET_ENVIRON_USERVAR,
      'E',
      'M',
      'P',
      'T',
      'Y',
      TELNET_ENVIRON_VALUE,
      TELNET_ENVIRON_USERVAR,
      'B',
      'I',
      'N',
      'A',
      'R',
      'Y',
      TELNET_ENVIRON_VALUE,
      'x',
      TELNET_ENVIRON_ESC,
      TELNET_ENVIRON_VALUE,
      'y',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'S',
      'E',
      'N',
      'D',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'P',
      'R',
      'O',
      'M',
      'P',
      'T',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'S',
      'T',
      'Y',
      'L',
      'E',
      '_',
      'B',
      'A',
      'S',
      'I',
      'C',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'S',
      'T',
      'Y',
      'L',
      'E',
      '_',
      'S',
      'T',
      'A',
      'T',
      'E',
      'S',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'T',
      'O',
      'O',
      'L',
      'T',
      'I',
      'P',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'M',
      'E',
      'N',
      'U',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'V',
      'I',
      'S',
      'I',
      'B',
      'I',
      'L',
      'I',
      'T',
      'Y',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'S',
      'P',
      'O',
      'I',
      'L',
      'E',
      'R',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'D',
      'I',
      'S',
      'A',
      'B',
      'L',
      'E',
      'D',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'S',
      'E',
      'L',
      'E',
      'C',
      'T',
      'I',
      'O',
      'N',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'C',
      'O',
      'M',
      'P',
      'A',
      'C',
      'T',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_ENVIRON_USERVAR,
      'O',
      'S',
      'C',
      '_',
      'H',
      'Y',
      'P',
      'E',
      'R',
      'L',
      'I',
      'N',
      'K',
      'S',
      '_',
      'P',
      'R',
      'E',
      'S',
      'E',
      'T',
      'S',
      TELNET_ENVIRON_VALUE,
      '1',
      TELNET_IAC,
      TELNET_SE,
  };
  if (send_bytes(client->socket_fd, enable, sizeof(enable)) < 0 ||
      wait_for_pattern(client, "NEW-ENVIRON request", request,
                       sizeof(request)) < 0)
    return -1;
  return send_bytes(client->socket_fd, response, sizeof(response));
}

static int send_ttype(int socket_fd, const char *value) {
  unsigned char response[128];
  size_t value_size = strlen(value);
  size_t size = 0;

  if (value_size + 6 > sizeof(response))
    return -1;
  append_byte(response, sizeof(response), &size, TELNET_IAC);
  append_byte(response, sizeof(response), &size, TELNET_SB);
  append_byte(response, sizeof(response), &size, TELNET_TTYPE);
  append_byte(response, sizeof(response), &size, TELNET_TTYPE_IS);
  memcpy(buffer_suffix(response, sizeof(response), size), value, value_size);
  size += value_size;
  append_byte(response, sizeof(response), &size, TELNET_IAC);
  append_byte(response, sizeof(response), &size, TELNET_SE);
  return send_bytes(socket_fd, response, size);
}

static int negotiate_mtts(TelnetTestClient *client) {
  static const unsigned char will_ttype[] = {
      TELNET_IAC,
      TELNET_WILL,
      TELNET_TTYPE,
  };

  if (send_bytes(client->socket_fd, will_ttype, sizeof(will_ttype)) < 0 ||
      expect_ttype_request(client) < 0 ||
      send_ttype(client->socket_fd, "MUDLET") < 0 ||
      expect_ttype_request(client) < 0 ||
      send_ttype(client->socket_fd, "XTERM") < 0 ||
      expect_ttype_request(client) < 0 ||
      send_ttype(client->socket_fd, "MTTS 329") < 0)
    return -1;
  return 0;
}

static int send_command(int socket_fd, const char *command) {
  return send_bytes(socket_fd, (const unsigned char *)command, strlen(command));
}

static int expect_text(int socket_fd, const char *expected) {
  char received[16384];
  size_t received_size = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
  int idle_attempts = 0;

  while (idle_attempts < 20) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1) {
      idle_attempts++;
      continue;
    }
    size = read(socket_fd,
                buffer_suffix(received, sizeof(received), received_size),
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    *(char *)checked_storage_at(received, sizeof(received), sizeof(char),
                                received_size) = '\0';
    if (strstr(received, expected))
      return 0;
    if (received_size == sizeof(received) - 1)
      break;
  }
  fprintf(stderr, "expected '%s', received '%s'\n", expected, received);
  return -1;
}

static int expect_text_without(int socket_fd, const char *expected,
                               const char *forbidden) {
  char received[16384];
  size_t received_size = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
  int idle_attempts = 0;

  while (idle_attempts < 20) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1) {
      idle_attempts++;
      continue;
    }
    size = read(socket_fd,
                buffer_suffix(received, sizeof(received), received_size),
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    *(char *)checked_storage_at(received, sizeof(received), sizeof(char),
                                received_size) = '\0';
    if (strstr(received, forbidden)) {
      fprintf(stderr, "received forbidden '%s' while expecting '%s': '%s'\n",
              forbidden, expected, received);
      return -1;
    }
    if (strstr(received, expected))
      return 0;
    if (received_size == sizeof(received) - 1)
      break;
  }
  fprintf(stderr, "expected '%s', received '%s'\n", expected, received);
  return -1;
}

static int expect_three_texts(int socket_fd, const char *first,
                              const char *second, const char *third) {
  char received[16384];
  size_t received_size = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
  int idle_attempts = 0;

  while (idle_attempts < 20) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1) {
      idle_attempts++;
      continue;
    }
    size = read(socket_fd,
                buffer_suffix(received, sizeof(received), received_size),
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    *(char *)checked_storage_at(received, sizeof(received), sizeof(char),
                                received_size) = '\0';
    if (strstr(received, first) && strstr(received, second) &&
        strstr(received, third))
      return 0;
    if (received_size == sizeof(received) - 1)
      break;
  }
  fprintf(stderr, "expected '%s', '%s', and '%s', received '%s'\n", first,
          second, third, received);
  return -1;
}

static int expect_texts(int socket_fd, const char *const *expected,
                        size_t expected_count) {
  char *received = nullptr;
  size_t received_size = 0;
  size_t received_capacity = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
  int idle_attempts = 0;

  while (idle_attempts < 20) {
    if (poll(&readable, 1, 500) != 1) {
      idle_attempts++;
      continue;
    }
    if (received_capacity - received_size < 4096) {
      size_t new_capacity = received_capacity ? received_capacity * 2 : 16384;
      char *expanded = realloc(received, new_capacity);

      if (!expanded) {
        free(received);
        return -1;
      }
      received = expanded;
      received_capacity = new_capacity;
    }
    ssize_t size = read(
        socket_fd, buffer_suffix(received, received_capacity, received_size),
        received_capacity - received_size - 1);

    if (size <= 0) {
      free(received);
      return -1;
    }
    received_size += (size_t)size;
    *(char *)checked_storage_at(received, received_capacity, sizeof(char),
                                received_size) = '\0';

    bool found_all = true;
    for (size_t index = 0; index < expected_count; index++) {
      const char *item = *(const char *const *)checked_storage_at_const(
          expected, expected_count, sizeof(*expected), index);

      if (!strstr(received, item)) {
        found_all = false;
        break;
      }
    }
    if (found_all) {
      free(received);
      return 0;
    }
  }
  for (size_t index = 0; index < expected_count; index++) {
    const char *item = *(const char *const *)checked_storage_at_const(
        expected, expected_count, sizeof(*expected), index);

    if (!strstr(received, item))
      fprintf(stderr, "expected '%s'\n", item);
  }
  fprintf(stderr, "received '%s'\n", received ? received : "");
  free(received);
  return -1;
}

static int exercise_utf8(int socket_fd) {
  return send_command(socket_fd, "say UTF caf\xc3\xa9 \xf0\x9f\x98\x80\r\n") <
                     0 ||
                 expect_text(socket_fd, "UTF caf\xc3\xa9 \xf0\x9f\x98\x80") <
                     0 ||
                 send_command(socket_fd, "say split caf\xc3") < 0 ||
                 send_command(socket_fd, "\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "split caf\xc3\xa9") < 0 ||
                 send_command(socket_fd, "say caf\xc3\xa9\b!\r\n") < 0 ||
                 expect_text(socket_fd, "caf!") < 0 ||
                 send_command(socket_fd, "say bad\xc0\xaf\r\n") < 0 ||
                 expect_text(socket_fd,
                             "Input must be printable, valid UTF-8.") < 0 ||
                 send_command(socket_fd, "@create UTF Caf\xc3\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "UTF Caf\xc3\xa9 created as object") <
                     0 ||
                 send_command(socket_fd, "@open Porte;caf\xc3\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "Opened.") < 0 ||
                 send_command(socket_fd, "caf\xc3\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "You can't go that way.") < 0 ||
                 send_command(socket_fd, "@name me=Jos\xc3\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "You can't use that name.") < 0 ||
                 send_command(socket_fd, "@alias me=Jos\xc3\xa9\r\n") < 0 ||
                 expect_text(socket_fd, "That's a silly name for a player!") <
                     0 ||
                 send_command(socket_fd, "@chan/create Caf\xc3\xa9\r\n") < 0 ||
                 expect_text(
                     socket_fd,
                     "Channel names must be printable ASCII without spaces.") <
                     0 ||
                 send_command(socket_fd, "addcom \xc3\xa9=Public\r\n") < 0 ||
                 expect_text(socket_fd,
                             "Channel aliases must be 1-5 printable ASCII") <
                     0 ||
                 send_command(socket_fd, ".create UTF macros\r\n") < 0 ||
                 expect_text(socket_fd, "created with description UTF macros") <
                     0 ||
                 send_command(socket_fd, ".def \xc3\xa9=x\r\n") < 0 ||
                 expect_text(
                     socket_fd,
                     "Aliases must contain only printable ASCII characters.") <
                     0 ||
                 send_command(socket_fd, ".def go=say macro caf\xc3\xa9\r\n") <
                     0 ||
                 expect_text(socket_fd,
                             "Macro go:say macro caf\xc3\xa9 defined.") < 0 ||
                 send_command(socket_fd, ".go\r\n") < 0 ||
                 expect_text(socket_fd, "macro caf\xc3\xa9") < 0
             ? -1
             : 0;
}

static int exercise_split_modules(int socket_fd) {
  return send_command(socket_fd, "@list commands\r\n") < 0 ||
                 expect_text(socket_fd, "Built-in commands:") < 0 ||
                 send_command(socket_fd, "luaevents\r\n") < 0 ||
                 expect_text(socket_fd, "LuaEvents first=1 startup=1 connect=1 "
                                        "order=first,startup") < 0 ||
                 send_command(socket_fd, "objectevents\r\n") < 0 ||
                 expect_text(socket_fd,
                             "ObjectEvents first=2 startup=2 "
                             "order=first,first,startup,startup") < 0 ||
                 send_command(socket_fd, "@lua/check\r\n") < 0 ||
                 expect_text(socket_fd, "All Lua module checks passed.") < 0 ||
                 send_command(socket_fd, "lualocknested\r\n") < 0 ||
                 expect_text(socket_fd, "LuaLock nested isolated") < 0 ||
                 send_command(socket_fd, "lualockverify\r\n") < 0 ||
                 expect_text(socket_fd, "LuaLock state verified") < 0 ||
                 send_command(socket_fd, "lualifecycle\r\n") < 0 ||
                 expect_text(socket_fd,
                             "LuaLifecycle room thing exit destroyed") < 0 ||
                 send_command(socket_fd, "luateleport\r\n") < 0 ||
                 expect_text(socket_fd, "LuaTeleport moved") < 0 ||
                 send_command(socket_fd, "luazone\r\n") < 0 ||
                 expect_text(socket_fd, "LuaZone assigned") < 0 ||
                 send_command(socket_fd, "luaaffiliation\r\n") < 0 ||
                 expect_text(socket_fd, "LuaAffiliation assigned") < 0 ||
                 send_command(
                     socket_fd,
                     "@examine Lua Persistently Affiliated Thing\r\n") < 0 ||
                 expect_texts(socket_fd,
                              (const char *const[]){
                                  "Zone:",
                                  "Affiliation: Lua Persistent Affiliation",
                              },
                              2) < 0 ||
                 send_command(socket_fd,
                              "@examine/debug Lua Persistently Affiliated "
                              "Thing\r\n") < 0 ||
                 expect_texts(socket_fd,
                              (const char *const[]){
                                  "Zone    =",
                                  "Affil.  =",
                              },
                              2) < 0 ||
                 send_command(socket_fd, "luadbck\r\n") < 0 ||
                 expect_text_without(socket_fd, "LuaDbck complete", "Done.") <
                     0 ||
                 send_command(socket_fd, "luastale\r\n") < 0 ||
                 expect_text(socket_fd, "LuaStale rejected") < 0 ||
                 send_command(socket_fd, "lualuaparent\r\n") < 0 ||
                 expect_text(socket_fd, "LuaParent assigned") < 0 ||
                 send_command(socket_fd, "luaflags\r\n") < 0 ||
                 expect_text(socket_fd, "LuaFlags constants changed") < 0 ||
                 send_command(socket_fd, "@flag Lua Safe=safe\r\n") < 0 ||
                 expect_text(socket_fd, "Lua Safe - SAFE set.") < 0 ||
                 send_command(socket_fd, "luasafedestroy\r\n") < 0 ||
                 expect_text(socket_fd, "LuaSafe protected overridden") < 0 ||
                 send_command(socket_fd,
                              "@lua/schedule global_logic/example.lua\r\n") <
                     0 ||
                 expect_text(socket_fd,
                             "Schedules for global_logic/example.lua:") < 0 ||
                 send_command(socket_fd, "flow-demo confirm\r\n") < 0 ||
                 expect_text(socket_fd, "Really do the thing? (y/n)") < 0 ||
                 send_command(socket_fd, "y\r\n") < 0 ||
                 expect_text(socket_fd, "Done.") < 0 ||
                 send_command(socket_fd, "@create MovementWidget\r\n") < 0 ||
                 expect_text(socket_fd, "MovementWidget created as object") <
                     0 ||
                 send_command(socket_fd, "drop MovementWidget\r\n") < 0 ||
                 expect_text(socket_fd, "Dropped.") < 0 ||
                 send_command(socket_fd, "get MovementWidget\r\n") < 0 ||
                 expect_text(socket_fd, "Taken.") < 0 ||
                 send_command(socket_fd, "@destroy MovementWidget\r\n") < 0 ||
                 expect_text(socket_fd,
                             "The object shakes and begins to crumble.") < 0 ||
                 send_command(socket_fd, "@open SplitExit\r\n") < 0 ||
                 expect_text(socket_fd, "Opened.") < 0 ||
                 send_command(socket_fd, "@link SplitExit=here\r\n") < 0 ||
                 expect_text(socket_fd, "Linked.") < 0 ||
                 send_command(socket_fd, "@destroy SplitExit\r\n") < 0 ||
                 expect_text(socket_fd,
                             "The exit shakes and begins to crumble.") < 0
             ? -1
             : 0;
}

static int create_styled_object(int socket_fd) {
  if (send_command(socket_fd, "GOD\r\n") < 0 ||
      expect_three_texts(socket_fd, "preset:osc8-demo-button?config=",
                         "preset:osc8-demo-danger?config=", "Password:") < 0 ||
      send_command(socket_fd, "btmuxr0x\r\n") < 0 ||
      expect_text(socket_fd, "Connected.") < 0 ||
      send_command(socket_fd, "color truecolor\r\n") < 0 ||
      expect_text(socket_fd, "Color mode set to truecolor.") < 0 ||
      send_command(socket_fd, "help color\r\n") < 0 ||
      expect_text(
          socket_fd,
          "Custom names work anywhere a predefined color is accepted, but "
          "cannot override a CSS/X11 name.") < 0 ||
      send_command(socket_fd, "color\r\n") < 0 ||
      expect_text(socket_fd, "Color mode: truecolor (override).") < 0 ||
      send_command(socket_fd, "@flag me=monitor\r\n") < 0 ||
      expect_text(socket_fd, " - MONITOR set.") < 0 ||
      send_command(socket_fd, "@flag me=!monitor\r\n") < 0 ||
      expect_text(socket_fd, " - MONITOR cleared.") < 0 ||
      send_command(socket_fd, "@flag me=aUd\r\n") < 0 ||
      expect_text(socket_fd, " - AUDIBLE set.") < 0 ||
      send_command(socket_fd, "@flag me=!AuD\r\n") < 0 ||
      expect_text(socket_fd, " - AUDIBLE cleared.") < 0 ||
      send_command(socket_fd, "@power me=IdLe\r\n") < 0 ||
      expect_text(socket_fd, "GOD - idle granted.") < 0 ||
      send_command(socket_fd, "@search power=IDlE\r\n") < 0 ||
      expect_text(socket_fd, "GOD(#1") < 0 ||
      send_command(socket_fd, "@power me=!iDlE\r\n") < 0 ||
      expect_text(socket_fd, "GOD - idle removed.") < 0 ||
      send_command(socket_fd, "@name me=LongConnectedPlayer\r\n") < 0 ||
      expect_text(socket_fd, "Name set.") < 0 ||
      send_command(socket_fd, "@who\r\n") < 0 ||
      expect_text_without(socket_fd, "LongConnectedPla",
                          "LongConnectedPlayer") < 0 ||
      send_command(socket_fd, "@session\r\n") < 0 ||
      expect_text_without(socket_fd, "LongConnectedPla",
                          "LongConnectedPlayer") < 0 ||
      send_command(socket_fd, "@name me=GOD\r\n") < 0 ||
      expect_text(socket_fd, "Name set.") < 0 ||
      send_command(socket_fd, "@flag/quiet me=monitor\r\n") < 0 ||
      expect_text(socket_fd, "Command @flag does not take switches.") < 0 ||
      send_command(socket_fd, "@set me=monitor\r\n") < 0 ||
      expect_text(socket_fd, "Huh?  (Type \"help\" for help.)") < 0 ||
      send_command(socket_fd, "say [fg=red]PublicPlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "You say \"PublicPlain\"", "\033[") < 0 ||
      send_command(socket_fd, "page GOD=[fg=red]PrivatePlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "PrivatePlain", "\033[") < 0 ||
      /* A bare "page <text>" replays the saved last-page list, which must
       * not write back into the command line buffers. */
      send_command(socket_fd, "page RepeatToLastTarget\r\n") < 0 ||
      expect_text(socket_fd, "You paged GOD with 'RepeatToLastTarget'.") < 0 ||
      send_command(socket_fd, "@chan/create StyledTest\r\n") < 0 ||
      expect_text(socket_fd, "Channel StyledTest created.") < 0 ||
      /* The later luacolor callback inspects and boots this membership. */
      send_command(socket_fd, "addcom st=StyledTest\r\n") < 0 ||
      expect_text(socket_fd, "Channel StyledTest added with alias st.") < 0 ||
      send_command(socket_fd, "st [fg=red]ChannelPlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "ChannelPlain", "\033[") < 0 ||
      send_command(socket_fd, "st :[fg=red]ChannelPosePlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "ChannelPosePlain", "\033[") < 0 ||
      send_command(socket_fd, "@chan/emit StyledTest=[fg=red "
                              "bg=white]AdministrativeStyled[/]\r\n") < 0 ||
      expect_text(socket_fd, "\033[38;2;255;0;0m\033[48;2;255;255;255m"
                             "AdministrativeStyled") < 0 ||
      send_command(socket_fd, "luacolor\r\n") < 0 ||
      expect_three_texts(socket_fd, "\033[38;2;255;112;0mLuaMarkup",
                         "\033]8;;https://example.com\033\\Web\033]8;;\033\\",
                         "\033]8;;send:cast%20fireball\033\\Cast\033]8;;\033\\ "
                         "\033]8;;prompt:look\033\\Edit\033]8;;\033\\") < 0 ||
      send_command(socket_fd, "osc8demo\r\n") < 0 ||
      expect_texts(
          socket_fd,
          (const char *const[]){
              "OSC 8 demonstration (Tier 1-6)",
              "\033]8;;prompt:say%20OSC%208%20works%21\033\\fill input",
              "%22t%22%3A%22Left-click%20to%20look%3B%20right-click%20for%20"
              "more%20actions%22",
              "%22v%22%3A%7B%22action%22%3A%22conceal%22%2C%22delay%22"
              "%3A500%7D",
              "%22sp%22%3Atrue%2C%22d%22%3Atrue",
              "%22d%22%3Atrue%7D",
              "%22sel%22%3A%7B%22group%22%3A%22difficulty%22%2C%22value"
              "%22%3A%22easy%22",
              "%22sel%22%3A%7B%22group%22%3A%22buffs%22%2C%22value%22%3A"
              "%22strength%22%2C%22exclusive%22%3Afalse",
              "%22sel%22%3A%7B%22group%22%3A%22following%22%2C%22value%22"
              "%3A%22news%22%2C%22toggle%22%3Afalse%2C%22selected%22%3Atrue",
              "preset=osc8-demo-button",
              "preset=osc8-demo-button&config=",
              "preset=osc8-demo-danger",
              "Hover and focus the styled links",
          },
          13) < 0 ||
      send_command(socket_fd, "@telnet GOD\r\n") < 0 ||
      expect_three_texts(socket_fd, "  NEW-ENVIRON:",
                         "      VAR \"USER\" = "
                         "\"alice\"",
                         "      USERVAR \"BINARY\" = \"x\\x01y\"") < 0 ||
      send_command(socket_fd, "luastate\r\n") < 0 ||
      expect_text(socket_fd, "LuaState 1") < 0 ||
      send_command(socket_fd, "luafail\r\nluastate\r\n") < 0 ||
      expect_text(socket_fd, "LuaState 2") < 0 ||
      send_command(socket_fd, "@examine me\r\n") < 0 ||
      expect_text_without(socket_fd,
                          "State namespaces:\r\n  audit: 1 value\r\n"
                          "  integration: 4 values",
                          "  (none)") < 0 ||
      send_command(socket_fd, "@state\r\n") < 0 ||
      expect_text(socket_fd,
                  "@state command switches:\r\n"
                  "  /examine  Inspect persistent object state.\r\n"
                  "  /set      Set or clear a state value.\r\n"
                  "  /wipe     Clear object state or one namespace.\r\n"
                  "  /copy     Copy a state value on an object.\r\n"
                  "  /move     Move a state value on an object.") < 0 ||
      send_command(socket_fd, "@state/examine\r\n") < 0 ||
      expect_text(socket_fd,
                  "State namespaces:\r\n  audit: 1 value\r\n"
                  "  integration: 4 values\r\n"
                  "Type @state/examine <object>/<namespace> to list the values "
                  "in a namespace.") < 0 ||
      send_command(socket_fd, "@state/examine me\r\n") < 0 ||
      expect_text(socket_fd,
                  "State namespaces:\r\n  audit: 1 value\r\n"
                  "  integration: 4 values\r\n"
                  "Type @state/examine <object>/<namespace> to list the values "
                  "in a namespace.") < 0 ||
      send_command(socket_fd, "@state/examine me/integration\r\n") < 0 ||
      expect_text(socket_fd, "State namespace integration:\r\n"
                             "  balance (integer): 2\r\n"
                             "  enabled (boolean): true\r\n"
                             "  memo (string): \"\"\r\n"
                             "  rate (number): 1.25") < 0 ||
      send_command(socket_fd, "@state/set here/integration empty=\"\"\r\n") <
          0 ||
      expect_text(socket_fd, "State value set.") < 0 ||
      send_command(socket_fd, "@state/set here/integration count=7\r\n") < 0 ||
      expect_text(socket_fd, "State value set.") < 0 ||
      send_command(socket_fd, "@state/set here/integration flag=false\r\n") <
          0 ||
      expect_text(socket_fd, "State value set.") < 0 ||
      send_command(socket_fd, "@state/set here/integration label=hello\r\n") <
          0 ||
      expect_text(socket_fd, "State value set.") < 0 ||
      send_command(socket_fd,
                   "@state/set here/integration temporary=gone\r\n") < 0 ||
      expect_text(socket_fd, "State value set.") < 0 ||
      send_command(socket_fd, "@state/set here/integration temporary=\r\n") <
          0 ||
      expect_text(socket_fd, "State value cleared.") < 0 ||
      send_command(socket_fd, "@state/copy here/integration balance=archive "
                              "copied_balance\r\n") < 0 ||
      expect_text(socket_fd, "State value copied.") < 0 ||
      send_command(socket_fd, "@state/move here/integration enabled=archive "
                              "moved_enabled\r\n") < 0 ||
      expect_text(socket_fd, "State value moved.") < 0 ||
      send_command(socket_fd, "@state/examine here/archive\r\n") < 0 ||
      expect_text(socket_fd, "State namespace archive:\r\n"
                             "  copied_balance (integer): 2\r\n"
                             "  moved_enabled (boolean): true") < 0 ||
      send_command(socket_fd, "@state/examine here/integration\r\n") < 0 ||
      expect_text(socket_fd, "State namespace integration:\r\n"
                             "  balance (integer): 2\r\n"
                             "  count (integer): 7\r\n"
                             "  empty (string): \"\"\r\n"
                             "  flag (boolean): false\r\n"
                             "  label (string): \"hello\"\r\n"
                             "  memo (string): \"\"\r\n"
                             "  rate (number): 1.25") < 0 ||
      send_command(socket_fd, "@state/wipe here/archive\r\n") < 0 ||
      expect_text(socket_fd, "2 state values wiped.") < 0 ||
      send_command(socket_fd, "@state/wipe here/audit\r\n") < 0 ||
      expect_text(socket_fd, "1 state value wiped.") < 0 ||
      send_command(socket_fd, "@state/wipe here\r\n") < 0 ||
      expect_text(socket_fd, "7 state values wiped.") < 0 ||
      send_command(socket_fd, "look\r\n") < 0 ||
      expect_text(socket_fd, "Starter Room") < 0) {
    fprintf(stderr, "styled-object login failed\n");
    return -1;
  }
  if (exercise_utf8(socket_fd) < 0) {
    fprintf(stderr, "UTF-8 behavior failed\n");
    return -1;
  }
  if (exercise_split_modules(socket_fd) < 0) {
    fprintf(stderr, "split-module behavior failed\n");
    return -1;
  }
  if (send_command(socket_fd, "@create [fg=#112233]StyledWidget[/]\r\n") < 0 ||
      expect_text(socket_fd, "\033[38;2;17;34;51mStyledWidget") < 0) {
    fprintf(stderr, "styled-object creation failed\n");
    return -1;
  }
  if (send_command(socket_fd,
                   "@name StyledWidget=[fg=bright-cyan]RenamedWidget[/]\r\n") <
          0 ||
      expect_text(socket_fd, "Name set.") < 0) {
    fprintf(stderr, "styled-object rename failed\n");
    return -1;
  }
  if (send_command(socket_fd,
                   "@attribute/set RenamedWidget/Description=[send=\"look\" "
                   "color=red bold "
                   "hover.color=yellow tooltip=\"Inspect this object\" "
                   "title=\"Actions\" menu.1.label=\"Look\" "
                   "menu.1.send=\"look\" menu.2.label=\"Examine\" "
                   "menu.2.prompt=\"examine RenamedWidget\" "
                   "visibility.action=conceal visibility.delay=500 "
                   "visibility.expire.prompt spoiler "
                   "selection.group=\"objects\" "
                   "selection.value=\"description\" selection.selected "
                   "selection.exclusive=false selection.disabled=false "
                   "disabled=false]Description[/]"
                   "\r\n") < 0 ||
      expect_text(socket_fd, "Description - Set.") < 0) {
    fprintf(stderr, "styled-object description failed\n");
    return -1;
  }
  if (send_command(socket_fd, "look RenamedWidget\r\n") < 0 ||
      expect_text(socket_fd,
                  "\033]8;;send:look?config=%7B%22s%22%3A%7B%22c%22"
                  "%3A%22%23ff0000%22%2C%22b%22%3Atrue%2C%22h%22%3A"
                  "%7B%22c%22%3A%22%23ffff00%22%7D%7D%2C%22t%22"
                  "%3A%22Inspect%20this%20object%22%2C%22m%22%3A%5B%7B"
                  "%22Look%22%3A%22send%3Alook%22%7D%2C%7B%22Examine%22%3A"
                  "%22prompt%3Aexamine%20RenamedWidget%22%7D%5D%2C%22ti"
                  "%22%3A%22Actions%22%2C%22v%22%3A%7B%22action%22"
                  "%3A%22conceal%22%2C%22delay%22%3A500%2C%22expire%22%3A"
                  "%7B%22prompt%22%3Atrue%7D%7D%2C%22sel%22%3A%7B%22"
                  "group%22%3A%22objects%22%2C%22value%22%3A%22description"
                  "%22%2C%22selected%22%3Atrue%2C%22exclusive%22%3Afalse%2C"
                  "%22disabled%22%3Afalse%7D%2C%22sp%22%3Atrue%2C"
                  "%22d%22%3Afalse%7D\033\\"
                  "Description\033]8;;\033\\") < 0) {
    fprintf(stderr, "OSC Tier 6 rendering failed\n");
    return -1;
  }
  if (send_command(socket_fd,
                   "@attribute/set RenamedWidget/InternalDescription="
                   "[bg=blue]Inside[/]\r\n") < 0 ||
      expect_text(socket_fd, "InternalDescription - Set.") < 0) {
    fprintf(stderr, "styled-object inside description failed\n");
    return -1;
  }
  if (send_command(socket_fd, "@attribute/get RenamedWidget/Desc\r\n") < 0 ||
      expect_text(socket_fd, "That is not an administrable attribute.") < 0 ||
      send_command(socket_fd, "@attribute/get RenamedWidget/Idesc\r\n") < 0 ||
      expect_text(socket_fd, "That is not an administrable attribute.") < 0) {
    fprintf(stderr, "legacy description attributes were accepted\n");
    return -1;
  }
  if (send_command(socket_fd, "@examine RenamedWidget\r\n") < 0 ||
      expect_three_texts(socket_fd, "[fg=bright-cyan]RenamedWidget[/](#",
                         "Description: [send=\"look\" color=red bold "
                         "hover.color=yellow tooltip=\"Inspect this object\" "
                         "title=\"Actions\" menu.1.label=\"Look\" "
                         "menu.1.send=\"look\" menu.2.label=\"Examine\" "
                         "menu.2.prompt=\"examine RenamedWidget\" "
                         "visibility.action=conceal visibility.delay=500 "
                         "visibility.expire.prompt spoiler "
                         "selection.group=\"objects\" "
                         "selection.value=\"description\" selection.selected "
                         "selection.exclusive=false selection.disabled=false "
                         "disabled=false]"
                         "Description[/]",
                         "InternalDescription: [bg=blue]Inside[/]") < 0) {
    fprintf(stderr, "styled-object examine markup failed\n");
    return -1;
  }
  return 0;
}

static int exercise_plain_osc_fallback(int socket_fd) {
  if (send_command(socket_fd, "abcdefghijklmnopqrstuvwxyz12345\r\n") < 0 ||
      expect_text(socket_fd, "New usernames may be at most ") < 0 ||
      send_command(socket_fd, "GOD\r\n") < 0 ||
      expect_text(socket_fd, "Password:") < 0 ||
      send_command(socket_fd, "btmuxr0x\r\n") < 0 ||
      expect_text(socket_fd, "Connected.") < 0 ||
      send_command(socket_fd, "osclinks\r\n") < 0 ||
      expect_text_without(socket_fd, "Web Cast Edit", "\033]") < 0) {
    fprintf(stderr, "OSC plain fallback failed\n");
    return -1;
  }
  if (send_command(socket_fd, "osc8demo\r\n") < 0 ||
      expect_text_without(socket_fd, "Tier 6 - presets:", "\033]") < 0) {
    fprintf(stderr, "OSC demo plain fallback failed\n");
    return -1;
  }
  return 0;
}

static int check_styled_object(const char *directory) {
  char database_path[PATH_MAX];
  sqlite3 *database = nullptr;
  sqlite3_stmt *statement = nullptr;
  int result = -1;

  snprintf(database_path, sizeof(database_path), "%s/data/stompymux.db",
           directory);
  if (sqlite3_open_v2(database_path, &database, SQLITE_OPEN_READONLY,
                      nullptr) != SQLITE_OK ||
      sqlite3_prepare_v2(
          database,
          "SELECT name, description, internal_description,"
          " (SELECT value FROM object_state WHERE object_dbref = 1"
          "  AND namespace = 'integration' AND key = 'balance'"
          "  AND value_type = 3),"
          " (SELECT count(*) FROM objects WHERE name = 'UTF Caf\xc3\xa9'),"
          " (SELECT count(*) FROM objects WHERE name = 'Porte;caf\xc3\xa9')"
          ", (SELECT location FROM objects"
          "   WHERE name = 'Lua Persistently Linked Exit')"
          ", (SELECT dbref FROM objects"
          "   WHERE name = 'Lua Link Destination Two')"
          ", (SELECT location FROM objects"
          "   WHERE name = 'Lua Persistently Unlinked Exit')"
          ", (SELECT location FROM objects"
          "   WHERE name = 'Lua Persistently Teleported Thing')"
          ", (SELECT dbref FROM objects"
          "   WHERE name = 'Lua Teleport Destination')"
          ", (SELECT zone FROM objects"
          "   WHERE name = 'Lua Persistently Zoned Thing')"
          ", (SELECT dbref FROM objects"
          "   WHERE name = 'Lua Persistent Zone')"
          ", (SELECT affiliation FROM objects"
          "   WHERE name = 'Lua Persistently Affiliated Thing')"
          ", (SELECT dbref FROM objects"
          "   WHERE name = 'Lua Persistent Affiliation')"
          ", (SELECT affiliation FROM objects"
          "   WHERE name = 'Lua Cleared Affiliation Target')"
          ", (SELECT lua_parent FROM objects"
          "   WHERE name = 'Lua Persistently Parented Thing')"
          ", (SELECT lua_parent FROM objects"
          "   WHERE name = 'Lua Persistently Unparented Thing')"
          " FROM objects WHERE name LIKE '%RenamedWidget%';",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_ROW)
    goto done;
  if (!strcmp((const char *)sqlite3_column_text(statement, 0),
              "[fg=bright-cyan]RenamedWidget[/]") &&
      !strcmp((const char *)sqlite3_column_text(statement, 1),
              "[send=\"look\" color=red bold "
              "hover.color=yellow tooltip=\"Inspect this object\" "
              "title=\"Actions\" menu.1.label=\"Look\" "
              "menu.1.send=\"look\" menu.2.label=\"Examine\" "
              "menu.2.prompt=\"examine RenamedWidget\" "
              "visibility.action=conceal visibility.delay=500 "
              "visibility.expire.prompt spoiler "
              "selection.group=\"objects\" "
              "selection.value=\"description\" selection.selected "
              "selection.exclusive=false selection.disabled=false "
              "disabled=false]Description[/]") &&
      !strcmp((const char *)sqlite3_column_text(statement, 2),
              "[bg=blue]Inside[/]") &&
      sqlite3_column_int64(statement, 3) == 2 &&
      sqlite3_column_int(statement, 4) == 1 &&
      sqlite3_column_int(statement, 5) == 1 &&
      sqlite3_column_type(statement, 6) != SQLITE_NULL &&
      sqlite3_column_type(statement, 7) != SQLITE_NULL &&
      sqlite3_column_int64(statement, 6) ==
          sqlite3_column_int64(statement, 7) &&
      sqlite3_column_int64(statement, 8) == -1 &&
      sqlite3_column_type(statement, 9) != SQLITE_NULL &&
      sqlite3_column_type(statement, 10) != SQLITE_NULL &&
      sqlite3_column_int64(statement, 9) ==
          sqlite3_column_int64(statement, 10) &&
      sqlite3_column_type(statement, 11) != SQLITE_NULL &&
      sqlite3_column_type(statement, 12) != SQLITE_NULL &&
      sqlite3_column_int64(statement, 11) ==
          sqlite3_column_int64(statement, 12) &&
      sqlite3_column_type(statement, 13) != SQLITE_NULL &&
      sqlite3_column_type(statement, 14) != SQLITE_NULL &&
      sqlite3_column_int64(statement, 13) ==
          sqlite3_column_int64(statement, 14) &&
      sqlite3_column_int64(statement, 15) == -1 &&
      !strcmp((const char *)sqlite3_column_text(statement, 16),
              "example.lua") &&
      !strcmp((const char *)sqlite3_column_text(statement, 17), ""))
    result = 0;

done:
  sqlite3_finalize(statement);
  sqlite3_close(database);
  return result;
}

/* Start a server, open enough clients to grow its registry, and stop it. */
int main(int argc, char **argv) {
  char target_config[PATH_MAX];
  int socket_fds[TEST_CONNECTION_COUNT];
  TelnetTestClient primary_client = {.socket_fd = -1};
  pid_t child = -1;
  int port;
  int result = 1;

  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++)
    *socket_slot(socket_fds, index) = -1;
  if (argc != 3)
    return 1;
  const char *directory = process_argument(argv, argc, 2);
  snprintf(target_config, sizeof(target_config), "%s/stompymux.toml",
           directory);
  port = choose_port();
  if (port < 0 || remove_game_database(directory) < 0 ||
      write_config(target_config, port, true) < 0)
    goto done;
  child = fork();
  if (child < 0)
    goto done;
  if (child == 0) {
    if (chdir(directory) < 0)
      _exit(127);
    if (setenv("BTECH_TEST_GOD_PASSWORD", "btmuxr0x", 1) < 0)
      _exit(127);
    char *server = process_argument(argv, argc, 1);
    execl(server, server, "stompymux.toml", nullptr);
    _exit(127);
  }
  if (wait_child_failure(child) < 0) {
    fprintf(stderr, "built-in color collision did not halt startup\n");
    goto done;
  }
  child = -1;
  if (write_config(target_config, port, false) < 0 ||
      write_lua_fixture(directory) < 0)
    goto done;

  child = fork();
  if (child < 0)
    goto done;
  if (child == 0) {
    if (chdir(directory) < 0)
      _exit(127);
    if (setenv("BTECH_TEST_GOD_PASSWORD", "btmuxr0x", 1) < 0)
      _exit(127);
    char *server = process_argument(argv, argc, 1);
    execl(server, server, "stompymux.toml", nullptr);
    _exit(127);
  }
  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++) {
    int *socket_fd = socket_slot(socket_fds, index);
    *socket_fd = connect_when_ready(port);
    if (*socket_fd < 0) {
      fprintf(stderr, "connection %zu failed\n", index);
      goto done;
    }
    TelnetTestClient connection = {.socket_fd = *socket_fd};
    static const char welcome[] = "This site is under construction!";
    static const unsigned char charset_offer[] = {TELNET_IAC, TELNET_WILL,
                                                  TELNET_CHARSET};
    const void *second = index == 0 ? charset_offer : nullptr;
    const size_t SECOND_SIZE = index == 0 ? sizeof(charset_offer) : 0;
    if (wait_for_patterns(&connection, "connection welcome", welcome,
                          sizeof(welcome) - 1, second, SECOND_SIZE) < 0) {
      fprintf(stderr, "connection %zu welcome failed\n", index);
      goto done;
    }
    if (index == 0)
      primary_client = connection;
    if (index == 0 && negotiate_utf8(&primary_client) < 0) {
      fprintf(stderr, "UTF-8 negotiation failed\n");
      goto done;
    }
    if (index == 0 && negotiate_new_environ(&primary_client) < 0) {
      fprintf(stderr, "NEW-ENVIRON negotiation failed\n");
      goto done;
    }
    if (index == 0 && negotiate_mtts(&primary_client) < 0) {
      fprintf(stderr, "MTTS negotiation failed\n");
      goto done;
    }
  }
  if (create_styled_object(*socket_slot(socket_fds, 0)) < 0 ||
      exercise_invalid_lock_keys(*socket_slot(socket_fds, 0), directory) < 0 ||
      exercise_plain_osc_fallback(*socket_slot(socket_fds, 1)) < 0)
    goto done;
  if (kill(child, SIGTERM) < 0 || wait_child(child) < 0)
    goto done;
  child = -1;
  if (check_styled_object(directory) < 0) {
    fprintf(stderr, "styled-object database check failed in %s\n", directory);
    goto done;
  }

  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++) {
    int *socket_fd = socket_slot(socket_fds, index);
    if (*socket_fd >= 0)
      close(*socket_fd);
    *socket_fd = -1;
  }
  child = fork();
  if (child < 0)
    goto done;
  if (child == 0) {
    if (chdir(directory) < 0)
      _exit(127);
    char *server = process_argument(argv, argc, 1);
    execl(server, server, "stompymux.toml", nullptr);
    _exit(127);
  }
  {
    int *socket_fd = socket_slot(socket_fds, 0);
    *socket_fd = connect_when_ready(port);
    TelnetTestClient restart_client = {.socket_fd = *socket_fd};

    if (*socket_fd < 0 ||
        wait_for_patterns(&restart_client, "restart welcome",
                          "This site is under construction!",
                          sizeof("This site is under construction!") - 1,
                          nullptr, 0) < 0 ||
        send_command(*socket_fd, "GOD\r\n") < 0 ||
        expect_text(*socket_fd, "Password:") < 0 ||
        send_command(*socket_fd, "btmuxr0x\r\n") < 0 ||
        expect_text(*socket_fd, "Connected.") < 0 ||
        send_command(*socket_fd, "luazonereloaded\r\n") < 0 ||
        expect_text(*socket_fd, "LuaZone reloaded") < 0 ||
        send_command(*socket_fd, "luaaffiliationreloaded\r\n") < 0 ||
        expect_text(*socket_fd, "LuaAffiliation reloaded") < 0 ||
        send_command(*socket_fd, "luaevents\r\n") < 0 ||
        expect_text(*socket_fd,
                    "LuaEvents first=0 startup=1 connect=1 order=startup") <
            0 ||
        send_command(*socket_fd, "objectevents\r\n") < 0 ||
        expect_text(*socket_fd, "ObjectEvents first=0 startup=2 "
                                "order=startup,startup") < 0) {
      fprintf(stderr, "startup lifecycle restart check failed\n");
      goto done;
    }
  }
  if (kill(child, SIGTERM) < 0 || wait_child(child) < 0)
    goto done;
  child = -1;
  result = 0;

done:
  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++) {
    const int socket_fd = *socket_slot(socket_fds, index);
    if (socket_fd >= 0)
      close(socket_fd);
  }
  if (child > 0) {
    kill(child, SIGKILL);
    waitpid(child, nullptr, 0);
  }
  return result;
}
