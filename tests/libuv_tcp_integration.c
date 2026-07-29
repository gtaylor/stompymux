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

/* Number of simultaneous clients used to exercise descriptor registry growth.
 */
constexpr size_t TEST_CONNECTION_COUNT = 20;
constexpr unsigned char TELNET_IAC = 255;
constexpr unsigned char TELNET_SB = 250;
constexpr unsigned char TELNET_SE = 240;
constexpr unsigned char TELNET_WILL = 251;
constexpr unsigned char TELNET_TTYPE = 24;
constexpr unsigned char TELNET_TTYPE_IS = 0;
constexpr unsigned char TELNET_TTYPE_SEND = 1;

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

/* Run a command with three arguments and return whether it succeeded. */
static int run_command(const char *command, const char *first,
                       const char *second, const char *third) {
  int status;
  pid_t child = fork();

  if (child < 0)
    return -1;
  if (child == 0) {
    execlp(command, command, first, second, third, nullptr);
    _exit(127);
  }
  return waitpid(child, &status, 0) == child && WIFEXITED(status) &&
                 WEXITSTATUS(status) == 0
             ? 0
             : -1;
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

/* Copy the game configuration while replacing its listening port. */
static int write_config(const char *source_path, const char *target_path,
                        int port) {
  FILE *source = fopen(source_path, "r");
  FILE *target = fopen(target_path, "w");
  char *line = nullptr;
  size_t line_size = 0;
  bool replaced = false;

  if (source == nullptr || target == nullptr)
    goto fail;
  while (getline(&line, &line_size, source) >= 0) {
    if (strncmp(line, "port =", 6) == 0) {
      fprintf(target, "port = %d\n", port);
      replaced = true;
    } else {
      fputs(line, target);
    }
  }
  free(line);
  return fclose(source) == 0 && fclose(target) == 0 && replaced ? 0 : -1;

fail:
  free(line);
  if (source != nullptr)
    fclose(source);
  if (target != nullptr)
    fclose(target);
  return -1;
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
        "        local styled = mux.style(\"LuaHex\", { foreground = "
        "\"stompy-orange\" })\n"
        "        assert(mux.text_width(styled) == 6)\n"
        "        assert(mux.strip_style(styled) == \"LuaHex\")\n"
        "        assert(mux.strip_style(mux.truncate_text(styled, 3)) == "
        "\"Lua\")\n"
        "        assert(mux.object_inside_description(ctx.enactor) == nil)\n"
        "        mux.notify(ctx.enactor, "
        "mux.markup(\"[fg=stompy-orange]LuaMarkup[/]\"))\n"
        "        return true\n"
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
    ssize_t result = write(socket_fd, bytes + sent, size - sent);

    if (result <= 0)
      return -1;
    sent += (size_t)result;
  }
  return 0;
}

static int expect_ttype_request(int socket_fd) {
  static const unsigned char request[] = {
      TELNET_IAC,        TELNET_SB,  TELNET_TTYPE,
      TELNET_TTYPE_SEND, TELNET_IAC, TELNET_SE,
  };
  unsigned char received[1024];
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};

  for (int attempt = 0; attempt < 10; attempt++) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1)
      continue;
    size = read(socket_fd, received, sizeof(received));
    if (size <= 0)
      return -1;
    if (memmem(received, (size_t)size, request, sizeof(request)) != nullptr)
      return 0;
  }
  return -1;
}

static int send_ttype(int socket_fd, const char *value) {
  unsigned char response[128];
  size_t value_size = strlen(value);
  size_t size = 0;

  if (value_size + 6 > sizeof(response))
    return -1;
  response[size++] = TELNET_IAC;
  response[size++] = TELNET_SB;
  response[size++] = TELNET_TTYPE;
  response[size++] = TELNET_TTYPE_IS;
  memcpy(response + size, value, value_size);
  size += value_size;
  response[size++] = TELNET_IAC;
  response[size++] = TELNET_SE;
  return send_bytes(socket_fd, response, size);
}

static int negotiate_mtts(int socket_fd) {
  static const unsigned char will_ttype[] = {
      TELNET_IAC,
      TELNET_WILL,
      TELNET_TTYPE,
  };

  if (send_bytes(socket_fd, will_ttype, sizeof(will_ttype)) < 0 ||
      expect_ttype_request(socket_fd) < 0 ||
      send_ttype(socket_fd, "MUDLET") < 0 ||
      expect_ttype_request(socket_fd) < 0 ||
      send_ttype(socket_fd, "XTERM") < 0 ||
      expect_ttype_request(socket_fd) < 0 ||
      send_ttype(socket_fd, "MTTS 329") < 0)
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

  for (int attempt = 0; attempt < 20; attempt++) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1)
      continue;
    size = read(socket_fd, received + received_size,
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    received[received_size] = '\0';
    if (strstr(received, expected))
      return 0;
    if (received_size == sizeof(received) - 1)
      return -1;
  }
  fprintf(stderr, "expected '%s', received '%s'\n", expected, received);
  return -1;
}

static int expect_text_without(int socket_fd, const char *expected,
                               const char *forbidden) {
  char received[16384];
  size_t received_size = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};

  for (int attempt = 0; attempt < 20; attempt++) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1)
      continue;
    size = read(socket_fd, received + received_size,
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    received[received_size] = '\0';
    if (strstr(received, forbidden)) {
      fprintf(stderr, "received forbidden '%s' while expecting '%s': '%s'\n",
              forbidden, expected, received);
      return -1;
    }
    if (strstr(received, expected))
      return 0;
    if (received_size == sizeof(received) - 1)
      return -1;
  }
  fprintf(stderr, "expected '%s', received '%s'\n", expected, received);
  return -1;
}

static int expect_three_texts(int socket_fd, const char *first,
                              const char *second, const char *third) {
  char received[16384];
  size_t received_size = 0;
  struct pollfd readable = {.fd = socket_fd, .events = POLLIN};

  for (int attempt = 0; attempt < 20; attempt++) {
    ssize_t size;

    if (poll(&readable, 1, 500) != 1)
      continue;
    size = read(socket_fd, received + received_size,
                sizeof(received) - received_size - 1);
    if (size <= 0)
      return -1;
    received_size += (size_t)size;
    received[received_size] = '\0';
    if (strstr(received, first) && strstr(received, second) &&
        strstr(received, third))
      return 0;
    if (received_size == sizeof(received) - 1)
      return -1;
  }
  return -1;
}

static int create_styled_object(int socket_fd) {
  if (send_command(socket_fd, "GOD\r\n") < 0 ||
      expect_text(socket_fd, "Password:") < 0 ||
      send_command(socket_fd, "btmuxr0x\r\n") < 0 ||
      expect_text(socket_fd, "Connected.") < 0 ||
      send_command(socket_fd, "color truecolor\r\n") < 0 ||
      expect_text(socket_fd, "Color mode set to truecolor.") < 0 ||
      send_command(socket_fd, "help color\r\n") < 0 ||
      expect_text(
          socket_fd,
          "Custom names work anywhere a predefined color is accepted.") < 0 ||
      send_command(socket_fd, "color\r\n") < 0 ||
      expect_text(socket_fd, "Color mode: truecolor (override).") < 0 ||
      send_command(socket_fd, "say [fg=red]PublicPlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "You say \"PublicPlain\"", "\033[") < 0 ||
      send_command(socket_fd, "page GOD=[fg=red]PrivatePlain[/]\r\n") < 0 ||
      expect_text_without(socket_fd, "PrivatePlain", "\033[") < 0 ||
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
      expect_text(socket_fd, "\033[38;2;205;0;0m\033[48;2;229;229;229m"
                             "AdministrativeStyled") < 0 ||
      send_command(socket_fd, "luacolor\r\n") < 0 ||
      expect_text(socket_fd, "\033[38;2;255;112;0mLuaMarkup") < 0 ||
      send_command(socket_fd, "look\r\n") < 0 ||
      expect_text(socket_fd, "Staff Nexus") < 0) {
    fprintf(stderr, "styled-object login failed\n");
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
                   "@desc RenamedWidget=[fg=red]Description[/]\r\n") < 0 ||
      expect_text(socket_fd, "Desc - Set.") < 0) {
    fprintf(stderr, "styled-object description failed\n");
    return -1;
  }
  if (send_command(socket_fd, "@idesc RenamedWidget=[bg=blue]Inside[/]\r\n") <
          0 ||
      expect_text(socket_fd, "Idesc - Set.") < 0) {
    fprintf(stderr, "styled-object inside description failed\n");
    return -1;
  }
  if (send_command(socket_fd, "@examine RenamedWidget\r\n") < 0 ||
      expect_three_texts(socket_fd, "[fg=bright-cyan]RenamedWidget[/](#",
                         "Desc: [fg=red]Description[/]",
                         "Idesc: [bg=blue]Inside[/]") < 0) {
    fprintf(stderr, "styled-object examine markup failed\n");
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
          "SELECT name, description, inside_description FROM objects "
          "WHERE name LIKE '%RenamedWidget%';",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_ROW)
    goto done;
  if (!strcmp((const char *)sqlite3_column_text(statement, 0),
              "[fg=bright-cyan]RenamedWidget[/]") &&
      !strcmp((const char *)sqlite3_column_text(statement, 1),
              "[fg=red]Description[/]") &&
      !strcmp((const char *)sqlite3_column_text(statement, 2),
              "[bg=blue]Inside[/]"))
    result = 0;

done:
  sqlite3_finalize(statement);
  sqlite3_close(database);
  return result;
}

/* Start a server, open enough clients to grow its registry, and stop it. */
int main(int argc, char **argv) {
  char directory[] = "/tmp/stompymux-libuv-XXXXXX";
  char source_config[PATH_MAX];
  char target_config[PATH_MAX];
  char copy_source[PATH_MAX];
  char received[512];
  struct pollfd readable;
  int socket_fds[TEST_CONNECTION_COUNT];
  pid_t child = -1;
  int port;
  int result = 1;

  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++)
    socket_fds[index] = -1;
  if (argc != 3)
    return 1;
  if (mkdtemp(directory) == nullptr)
    return 1;
  snprintf(copy_source, sizeof(copy_source), "%s/.", argv[2]);
  if (run_command("cp", "-a", copy_source, directory) < 0)
    goto done;
  snprintf(source_config, sizeof(source_config), "%s/stompymux.toml", argv[2]);
  snprintf(target_config, sizeof(target_config), "%s/stompymux.toml",
           directory);
  port = choose_port();
  if (port < 0 || write_config(source_config, target_config, port) < 0 ||
      write_lua_fixture(directory) < 0)
    goto done;

  child = fork();
  if (child < 0)
    goto done;
  if (child == 0) {
    if (chdir(directory) < 0)
      _exit(127);
    execl(argv[1], argv[1], "stompymux.toml", nullptr);
    _exit(127);
  }
  for (size_t index = 0; index < TEST_CONNECTION_COUNT; index++) {
    socket_fds[index] = connect_when_ready(port);
    if (socket_fds[index] < 0) {
      fprintf(stderr, "connection %zu failed\n", index);
      goto done;
    }
    readable = (struct pollfd){.fd = socket_fds[index], .events = POLLIN};
    if (poll(&readable, 1, 5000) != 1 ||
        read(socket_fds[index], received, sizeof(received)) <= 0) {
      fprintf(stderr, "connection %zu welcome failed\n", index);
      goto done;
    }
    if (index == 0 && negotiate_mtts(socket_fds[index]) < 0) {
      fprintf(stderr, "MTTS negotiation failed\n");
      goto done;
    }
    if (index == 0 && create_styled_object(socket_fds[index]) < 0)
      goto done;
  }
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
    if (socket_fds[index] >= 0)
      close(socket_fds[index]);
  }
  if (child > 0) {
    kill(child, SIGKILL);
    waitpid(child, nullptr, 0);
  }
  run_command("rm", "-rf", "--", directory);
  return result;
}
