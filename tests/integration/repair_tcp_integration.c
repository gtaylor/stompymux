/* Authenticated repair command, event, and SQLite reload integration test. */

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
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "mux/support/checked_storage.h"

enum {
  TEST_TIMEOUT_MS = 15000,
  TEST_PROCESS_TIMEOUT_SECONDS = 60,
  COMMAND_POLL_MS = 100,
  COMMAND_INTERVAL_MS = 250,
  REPAIR_FIX_EVENT = 47,
  REPAIR_LOCATION = 7,
  REPAIR_PLAYER = 1,
  PAYLOAD_LOCATION_COUNT = 16,
  PAYLOAD_POSITION_COUNT = 16,
  PAYLOAD_EXTRA_COUNT = 256,
  PAYLOAD_PLAYER_FACTOR =
      PAYLOAD_LOCATION_COUNT * PAYLOAD_POSITION_COUNT * PAYLOAD_EXTRA_COUNT,
};

typedef struct RepairCheckpoint {
  int armor;
  int inventory;
  int event_count;
  int event_type;
  int remaining_ticks;
  long event_data;
  int is_fake;
} RepairCheckpoint;

static void *buffer_suffix(void *buffer, size_t capacity, size_t offset) {
  return checked_storage_region(buffer, capacity, offset, capacity - offset);
}

static const void *constant_buffer_suffix(const void *buffer, size_t capacity,
                                          size_t offset) {
  return checked_storage_region_const(buffer, capacity, offset,
                                      capacity - offset);
}

static bool buffer_contains(const char *buffer, size_t buffer_size,
                            const char *expected) {
  size_t expected_size = strlen(expected);
  if (expected_size > buffer_size)
    return false;
  for (size_t offset = 0; offset <= buffer_size - expected_size; offset++) {
    const char *candidate =
        checked_storage_at_const(buffer, buffer_size, sizeof(*buffer), offset);
    if (memcmp(candidate, expected, expected_size) == 0)
      return true;
  }
  return false;
}

static int choose_port(void) {
  struct sockaddr_in address = {.sin_family = AF_INET,
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
  socklen_t length = sizeof(address);
  int socket_fd = socket(AF_INET, SOCK_STREAM, 0);

  if (socket_fd < 0) {
    perror("repair TCP socket");
    return -1;
  }
  if (bind(socket_fd, (struct sockaddr *)&address, sizeof(address)) < 0) {
    perror("repair TCP bind");
    close(socket_fd);
    return -1;
  }
  if (getsockname(socket_fd, (struct sockaddr *)&address, &length) < 0) {
    perror("repair TCP getsockname");
    close(socket_fd);
    return -1;
  }
  close(socket_fd);
  return ntohs(address.sin_port);
}

static int connect_when_ready(int port) {
  struct sockaddr_in address = {.sin_family = AF_INET,
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK),
                                .sin_port = htons((uint16_t)port)};

  for (int attempt = 0; attempt < 100; ++attempt) {
    struct timespec delay = {.tv_nsec = 100000000};
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

static int stop_server(pid_t *child) {
  int status;
  pid_t running = *child;

  if (kill(running, SIGTERM) < 0)
    return -1;
  for (int attempt = 0; attempt < 100; ++attempt) {
    struct timespec delay = {.tv_nsec = 50000000};
    pid_t waited = waitpid(running, &status, WNOHANG);

    if (waited == running) {
      *child = -1;
      return WIFEXITED(status) && WEXITSTATUS(status) == 0 ? 0 : -1;
    }
    if (waited < 0)
      return -1;
    nanosleep(&delay, nullptr);
  }
  kill(running, SIGKILL);
  waitpid(running, &status, 0);
  *child = -1;
  return -1;
}

static pid_t start_server(const char *server, const char *directory,
                          bool bootstrap) {
  pid_t child = fork();

  if (child != 0)
    return child;
  if (chdir(directory) < 0 ||
      setenv("BTECH_TEST_GOD_PASSWORD", "btmuxr0x", 1) < 0 ||
      (bootstrap && setenv("BTECH_TEST_BTECH_BOOTSTRAP", "1", 1) < 0))
    _exit(127);
  execl(server, server, "stompymux.toml", nullptr);
  _exit(127);
}

static int send_text(int socket_fd, const char *text) {
  size_t sent = 0;
  size_t length = strlen(text);

  while (sent < length) {
    ssize_t written =
        write(socket_fd, constant_buffer_suffix(text, length + 1, sent),
              length - sent);
    if (written <= 0)
      return -1;
    sent += (size_t)written;
  }
  return 0;
}

static int expect_text(int socket_fd, const char *expected) {
  char received[16384] = {};
  size_t used = 0;

  for (int elapsed = 0; elapsed < TEST_TIMEOUT_MS; elapsed += 100) {
    struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
    int polled = poll(&readable, 1, 100);
    if (polled < 0 && errno == EINTR)
      continue;
    if (polled < 0)
      return -1;
    if (polled == 0)
      continue;
    ssize_t read_size =
        read(socket_fd, buffer_suffix(received, sizeof(received), used),
             sizeof(received) - used - 1);
    if (read_size <= 0)
      return -1;
    used += (size_t)read_size;
    *(char *)checked_storage_at(received, sizeof(received), sizeof(char),
                                used) = '\0';
    if (buffer_contains(received, used, expected))
      return 0;
  }
  fprintf(stderr, "expected '%s', received '%s'\n", expected, received);
  return -1;
}

static int64_t monotonic_milliseconds(void) {
  struct timespec current;

  if (clock_gettime(CLOCK_MONOTONIC, &current) < 0)
    return -1;
  return (int64_t)current.tv_sec * 1000 + current.tv_nsec / 1000000;
}

static int send_command_until_text(int socket_fd, const char *command,
                                   const char *expected) {
  char received[32768] = {};
  size_t used = 0;
  int64_t started = monotonic_milliseconds();
  int64_t next_command = started;

  while (started >= 0) {
    int64_t now = monotonic_milliseconds();
    if (now < 0 || now - started >= TEST_TIMEOUT_MS)
      break;
    if (now >= next_command) {
      if (send_text(socket_fd, command) < 0)
        return -1;
      next_command = now + COMMAND_INTERVAL_MS;
    }
    int wait_ms = (int)(next_command - now);
    if (wait_ms < 1)
      wait_ms = 1;
    if (wait_ms > COMMAND_POLL_MS)
      wait_ms = COMMAND_POLL_MS;
    struct pollfd readable = {.fd = socket_fd, .events = POLLIN};
    int polled = poll(&readable, 1, wait_ms);
    if (polled < 0 && errno == EINTR)
      continue;
    if (polled < 0)
      return -1;
    if (polled == 0)
      continue;
    if (used == sizeof(received) - 1)
      used = 0;
    ssize_t read_size =
        read(socket_fd, buffer_suffix(received, sizeof(received), used),
             sizeof(received) - used - 1);
    if (read_size <= 0)
      return -1;
    used += (size_t)read_size;
    *(char *)checked_storage_at(received, sizeof(received), sizeof(char),
                                used) = '\0';
    if (buffer_contains(received, used, expected))
      return 0;
  }
  fprintf(stderr, "command '%s' never produced '%s'; received '%s'\n", command,
          expected, received);
  return -1;
}

static int login_as_god(int socket_fd) {
  return expect_text(socket_fd, "Who are you?") < 0 ||
                 send_text(socket_fd, "GOD\r\n") < 0 ||
                 expect_text(socket_fd, "Password:") < 0 ||
                 send_text(socket_fd, "btmuxr0x\r\n") < 0 ||
                 expect_text(socket_fd, "Connected.") < 0
             ? -1
             : 0;
}

static int write_config(const char *path, int port) {
  FILE *file = fopen(path, "w");
  int result;

  if (!file)
    return -1;
  result = fprintf(file,
                   "[database]\n"
                   "game_database = \"data/stompymux.db\"\n"
                   "[battletech]\n"
                   "techtime_multiplier = 0.025\n"
                   "[server]\n"
                   "port = %d\n",
                   port) < 0 ||
                   fclose(file) != 0
               ? -1
               : 0;
  return result;
}

static int seed_special_mech(const char *database) {
  sqlite3 *sqlite = nullptr;
  char *error = nullptr;
  int result =
      sqlite3_open_v2(database, &sqlite, SQLITE_OPEN_READWRITE, nullptr) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "UPDATE snapshot SET db_top = 8 WHERE id = 1;"
                  "INSERT INTO objects "
                  "(dbref, name, location, contents, exits, next, "
                  "link, zone, type, has_xcode_flag) VALUES "
                  "(6, 'Repair Test Mech', 7, 1, -1, -1, -1, -1, 1, 1),"
                  "(7, 'Repair Test Store', -1, 6, -1, -1, -1, -1, 1, 0);"
                  "INSERT INTO btech_object_state "
                  "(object_dbref, object_type) VALUES (6, 'MECH');"
                  "UPDATE objects SET location = 6, next = -1 WHERE dbref = 1;",
                  nullptr, nullptr, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "BTech mech seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

static int seed_repair_state(const char *database) {
  sqlite3 *sqlite = nullptr;
  char *error = nullptr;
  int result =
      sqlite3_open_v2(database, &sqlite, SQLITE_OPEN_READWRITE, nullptr) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "UPDATE btech_mech_sections SET internal = 1, "
                  "internal_original = 1 WHERE mech_dbref = 6;"
                  "UPDATE btech_mech_sections SET armor = 0, "
                  "armor_original = 2, internal = 1, internal_original = 1 "
                  "WHERE mech_dbref = 6 AND section = 7;"
                  "INSERT OR REPLACE INTO btech_economy_parts "
                  "(object_dbref, part_id, brand_id, quantity) "
                  "VALUES (7, 547, 0, 2);"
                  "INSERT OR REPLACE INTO btech_character_state "
                  "(player_dbref, bruise, lethal, build, reflexes, intuition, "
                  "learn, charisma) "
                  "VALUES (1, 0, 0, 0, 0, 255, 255, 0);"
                  "INSERT OR REPLACE INTO btech_character_values "
                  "(player_dbref, value_name, value, xp, last_used) "
                  "VALUES (1, 'Technician-Battlemech', 255, 0, 0);",
                  nullptr, nullptr, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "Repair state seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

static int read_repair_checkpoint(const char *database,
                                  RepairCheckpoint *checkpoint) {
  sqlite3 *sqlite = nullptr;
  sqlite3_stmt *statement = nullptr;
  int result = -1;

  if (sqlite3_open_v2(database, &sqlite, SQLITE_OPEN_READONLY, nullptr) !=
          SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "SELECT "
          "(SELECT armor FROM btech_mech_sections WHERE mech_dbref = 6 AND "
          "section = 7), "
          "(SELECT quantity FROM btech_economy_parts WHERE object_dbref = 7 "
          "AND part_id = 547 AND brand_id = 0), "
          "(SELECT count(*) FROM btech_repair_events WHERE mech_dbref = 6), "
          "(SELECT event_type FROM btech_repair_events WHERE mech_dbref = 6), "
          "(SELECT remaining_ticks FROM btech_repair_events "
          "WHERE mech_dbref = 6), "
          "(SELECT event_data FROM btech_repair_events WHERE mech_dbref = 6), "
          "(SELECT is_fake FROM btech_repair_events WHERE mech_dbref = 6);",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_ROW)
    goto done;
  *checkpoint = (RepairCheckpoint){
      .armor = sqlite3_column_int(statement, 0),
      .inventory = sqlite3_column_int(statement, 1),
      .event_count = sqlite3_column_int(statement, 2),
      .event_type = sqlite3_column_int(statement, 3),
      .remaining_ticks = sqlite3_column_int(statement, 4),
      .event_data = sqlite3_column_int64(statement, 5),
      .is_fake = sqlite3_column_int(statement, 6),
  };
  result = 0;
done:
  sqlite3_finalize(statement);
  sqlite3_close(sqlite);
  return result;
}

static bool checkpoint_has_repair(const RepairCheckpoint *checkpoint, int armor,
                                  int amount) {
  long local_payload = checkpoint->event_data % PAYLOAD_PLAYER_FACTOR;
  int location = (int)(local_payload % PAYLOAD_LOCATION_COUNT);
  int position =
      (int)((local_payload / PAYLOAD_LOCATION_COUNT) % PAYLOAD_POSITION_COUNT);
  int extra =
      (int)(local_payload / (PAYLOAD_LOCATION_COUNT * PAYLOAD_POSITION_COUNT));
  int payload_amount =
      position + (extra % PAYLOAD_POSITION_COUNT) * PAYLOAD_POSITION_COUNT;
  long player = checkpoint->event_data / PAYLOAD_PLAYER_FACTOR;

  return checkpoint->armor == armor && checkpoint->inventory == 0 &&
         checkpoint->event_count == 1 &&
         checkpoint->event_type == REPAIR_FIX_EVENT &&
         checkpoint->remaining_ticks > 0 && checkpoint->is_fake == 0 &&
         location == REPAIR_LOCATION && payload_amount == amount &&
         player == REPAIR_PLAYER;
}

static bool checkpoint_is_complete(const RepairCheckpoint *checkpoint) {
  return checkpoint->armor == 2 && checkpoint->inventory == 0 &&
         checkpoint->event_count == 0;
}

static int require_repair_checkpoint(const char *phase,
                                     const RepairCheckpoint *checkpoint,
                                     int armor, int amount) {
  if (checkpoint_has_repair(checkpoint, armor, amount))
    return 0;
  fprintf(stderr,
          "%s checkpoint mismatch: armor=%d inventory=%d events=%d type=%d "
          "remaining=%d payload=%ld fake=%d\n",
          phase, checkpoint->armor, checkpoint->inventory,
          checkpoint->event_count, checkpoint->event_type,
          checkpoint->remaining_ticks, checkpoint->event_data,
          checkpoint->is_fake);
  return -1;
}

static int require_complete_checkpoint(const char *phase,
                                       const RepairCheckpoint *checkpoint) {
  if (checkpoint_is_complete(checkpoint))
    return 0;
  fprintf(stderr,
          "%s checkpoint mismatch: armor=%d inventory=%d events=%d type=%d "
          "remaining=%d payload=%ld fake=%d\n",
          phase, checkpoint->armor, checkpoint->inventory,
          checkpoint->event_count, checkpoint->event_type,
          checkpoint->remaining_ticks, checkpoint->event_data,
          checkpoint->is_fake);
  return -1;
}

int main(int argc, char **argv) {
  char config[PATH_MAX];
  char database[PATH_MAX];
  const char *directory;
  const char *server;
  int client = -1;
  int port;
  pid_t child = -1;
  int result = 1;
  RepairCheckpoint checkpoint;

  if (argc != 3)
    return 2;
  alarm(TEST_PROCESS_TIMEOUT_SECONDS);
  server = *(const char *const *)checked_storage_at_const(argv, (size_t)argc,
                                                          sizeof(*argv), 1);
  directory = *(const char *const *)checked_storage_at_const(argv, (size_t)argc,
                                                             sizeof(*argv), 2);
  if (snprintf(config, sizeof(config), "%s/stompymux.toml", directory) >=
          (int)sizeof(config) ||
      snprintf(database, sizeof(database), "%s/data/stompymux.db", directory) >=
          (int)sizeof(database) ||
      (port = choose_port()) < 0 || write_config(config, port) < 0)
    goto done;

  fprintf(stderr, "repair TCP phase: bootstrap database\n");
  child = start_server(server, directory, false);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      seed_special_mech(database) < 0)
    goto done;
  client = -1;
  fprintf(stderr, "repair TCP phase: bootstrap BTech state\n");
  child = start_server(server, directory, true);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      seed_repair_state(database) < 0)
    goto done;
  client = -1;
  fprintf(stderr, "repair TCP phase: schedule and persist two-point repair\n");
  child = start_server(server, directory, false);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      login_as_god(client) < 0 || send_text(client, "fixarmor head\r\n") < 0 ||
      expect_text(client, "You start fixing the armor..") < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      read_repair_checkpoint(database, &checkpoint) < 0 ||
      require_repair_checkpoint("pre-callback", &checkpoint, 0, 2) < 0)
    goto done;
  client = -1;
  fprintf(stderr, "repair TCP phase: restore and persist recursive callback\n");
  child = start_server(server, directory, false);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      login_as_god(client) < 0 || send_text(client, "fixarmor head\r\n") < 0 ||
      expect_text(client, "Someone's repairing that section already!") < 0 ||
      send_command_until_text(client, "damages\r\n",
                              "Repairs on armor (1 points)") < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      read_repair_checkpoint(database, &checkpoint) < 0 ||
      require_repair_checkpoint("intermediate", &checkpoint, 1, 1) < 0)
    goto done;
  client = -1;
  fprintf(stderr, "repair TCP phase: restore and complete once\n");
  child = start_server(server, directory, false);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      login_as_god(client) < 0 || send_text(client, "fixarmor head\r\n") < 0 ||
      expect_text(client, "Someone's repairing that section already!") < 0 ||
      expect_text(client, "armor repairs have been finished.") < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      read_repair_checkpoint(database, &checkpoint) < 0 ||
      require_complete_checkpoint("completed", &checkpoint) < 0)
    goto done;
  client = -1;
  fprintf(stderr, "repair TCP phase: restart completed snapshot\n");
  child = start_server(server, directory, false);
  if (child < 0 || (client = connect_when_ready(port)) < 0 ||
      close(client) < 0 || stop_server(&child) < 0 ||
      read_repair_checkpoint(database, &checkpoint) < 0 ||
      require_complete_checkpoint("restart", &checkpoint) < 0)
    goto done;
  result = 0;
done:
  if (client >= 0)
    close(client);
  if (child > 0)
    kill(child, SIGKILL);
  if (child > 0)
    waitpid(child, nullptr, 0);
  return result;
}
