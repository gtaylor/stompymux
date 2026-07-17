/* Process-level smoke test for the libuv TCP listener and shutdown path. */

#include <arpa/inet.h>
#include <errno.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
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

int main(int argc, char **argv) {
  char directory[] = "/tmp/stompymux-libuv-XXXXXX";
  char source_config[PATH_MAX];
  char target_config[PATH_MAX];
  char copy_source[PATH_MAX];
  char received[512];
  struct pollfd readable;
  pid_t child = -1;
  int port;
  int socket_fd = -1;
  int result = 1;

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
  if (port < 0 || write_config(source_config, target_config, port) < 0)
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
  socket_fd = connect_when_ready(port);
  if (socket_fd < 0)
    goto done;
  readable = (struct pollfd){.fd = socket_fd, .events = POLLIN};
  if (poll(&readable, 1, 5000) != 1 ||
      read(socket_fd, received, sizeof(received)) <= 0)
    goto done;
  if (kill(child, SIGTERM) < 0 || wait_child(child) < 0)
    goto done;
  child = -1;
  result = 0;

done:
  if (socket_fd >= 0)
    close(socket_fd);
  if (child > 0) {
    kill(child, SIGKILL);
    waitpid(child, nullptr, 0);
  }
  run_command("rm", "-rf", "--", directory);
  return result;
}
