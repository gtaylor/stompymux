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

static int write_lua_fixture(const char *directory) {
  char path[PATH_MAX];
  FILE *file;

  snprintf(path, sizeof(path), "%s/lua/global_logic/styled_text_test.lua",
           directory);
  file = fopen(path, "w");
  if (!file)
    return -1;
  fputs("return {\n"
        "  commands = {\n"
        "    {\n"
        "      pattern = \"^luacolor$\",\n"
        "      handler = function(ctx)\n"
        "        assert(mux.object == nil)\n"
        "        assert(mux.connected_players == nil)\n"
        "        assert(mux.who_summary == nil)\n"
        "        assert(mux.flow_start == nil)\n"
        "        assert(mux.notify == nil)\n"
        "        assert(mux.world.connected_players == nil)\n"
        "        assert(mux.world.who_summary == nil)\n"
        "        assert(type(mux.world.pemit) == \"function\")\n"
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
      "        assert(player.dbref == ctx.enactor)\n"
      "        assert(player.type == \"player\")\n"
      "        assert(player.inside_description == nil)\n"
      "        local invalid_ok, invalid_error = "
      "pcall(mux.world.object, 999999)\n"
      "        assert(not invalid_ok)\n"
      "        assert(invalid_error.code == \"mux.object.invalid\")\n"
      "        local checked_ok, checked_player = "
      "mux.error.pcall(mux.world.object, ctx.enactor)\n"
      "        assert(checked_ok)\n"
      "        assert(checked_player.dbref == ctx.enactor)\n"
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
  fputs(
      "    {\n"
      "      pattern = \"^osclinks$\",\n"
      "      handler = function(ctx)\n"
      "        mux.world.pemit(ctx.enactor, mux.text.markup("
      "\"[link=\\\"https://example.com\\\"]Web[/] \" .."
      " \"[send=\\\"cast fireball\\\"]Cast[/] \" .."
      " \"[prompt=\\\"look\\\"]Edit[/]\"))\n"
      "        return true\n"
      "      end,\n"
      "    },\n"
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
                 send_command(socket_fd, "@lua/check\r\n") < 0 ||
                 expect_text(socket_fd, "All Lua module checks passed.") < 0 ||
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
  if (send_command(
          socket_fd,
          "@attribute/set RenamedWidget/Desc=[send=\"look\" color=red bold "
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
      expect_text(socket_fd, "Desc - Set.") < 0) {
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
  if (send_command(
          socket_fd,
          "@attribute/set RenamedWidget/Idesc=[bg=blue]Inside[/]\r\n") < 0 ||
      expect_text(socket_fd, "Idesc - Set.") < 0) {
    fprintf(stderr, "styled-object inside description failed\n");
    return -1;
  }
  if (send_command(socket_fd, "@examine RenamedWidget\r\n") < 0 ||
      expect_three_texts(socket_fd, "[fg=bright-cyan]RenamedWidget[/](#",
                         "Desc: [send=\"look\" color=red bold "
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
                         "Idesc: [bg=blue]Inside[/]") < 0) {
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
          "SELECT name, description, inside_description,"
          " (SELECT value FROM object_state WHERE object_dbref = 1"
          "  AND namespace = 'integration' AND key = 'balance'"
          "  AND value_type = 3),"
          " (SELECT count(*) FROM objects WHERE name = 'UTF Caf\xc3\xa9'),"
          " (SELECT count(*) FROM objects WHERE name = 'Porte;caf\xc3\xa9')"
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
      sqlite3_column_int(statement, 5) == 1)
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
      exercise_plain_osc_fallback(*socket_slot(socket_fds, 1)) < 0)
    goto done;
  if (kill(child, SIGTERM) < 0 || wait_child(child) < 0)
    goto done;
  child = -1;
  if (check_styled_object(directory) < 0) {
    fprintf(stderr, "styled-object database check failed in %s\n", directory);
    goto done;
  }
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
