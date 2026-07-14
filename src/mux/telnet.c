#include "config.h"

#include <ctype.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "command.h"
#include "debug.h"
#include "externs.h"
#include "interface.h"
#include "libtelnet.h"
#include "logcache.h"
#include "mudconf.h"
#include "telnet.h"
#include "telnet_socket.h"

static int telnet_connected_count(void);
static int telnet_charset_is_ascii(const char *buffer, size_t size);
static void telnet_process_data(DESC *d, const char *buffer, size_t size);
static void telnet_handle_charset(DESC *d, const char *buffer, size_t size);
static void telnet_send_charset_accepted(telnet_t *telnet);
static void telnet_send_charset_rejected(telnet_t *telnet);
static void telnet_send_charset_request(DESC *d);
static void telnet_handle_gmcp(DESC *d, const char *buffer, size_t size);
static void telnet_send_gmcp(telnet_t *telnet, const char *package);
static void telnet_send_mssp(telnet_t *telnet);
static void telnet_send_mssp_pair(telnet_t *telnet, const char *name,
                                  const char *value);
static void telnet_event_handler(telnet_t *telnet, telnet_event_t *event,
                                 void *user_data);

enum {
  telnet_charset_option = 42,
  telnet_charset_request = 1,
  telnet_charset_accepted = 2,
  telnet_charset_rejected = 3,
  telnet_gmcp_option = 201,
};

static const telnet_telopt_t telnet_options[] = {
    {TELNET_TELOPT_TTYPE, TELNET_WONT, TELNET_DO},
    {TELNET_TELOPT_NAWS, TELNET_WONT, TELNET_DO},
    {TELNET_TELOPT_MSSP, TELNET_WILL, TELNET_DONT},
    {telnet_charset_option, TELNET_WILL, TELNET_DONT},
    {telnet_gmcp_option, TELNET_WILL, TELNET_DONT},
    {-1, 0, 0},
};

int telnet_initialize(DESC *d) {
  d->telnet =
      telnet_init(telnet_options, telnet_event_handler, TELNET_FLAG_NVT_EOL, d);
  if (d->telnet == NULL) {
    log_error(LOG_PROBLEMS, "TELNET", "ERROR",
              "Unable to allocate Telnet state for descriptor %d.",
              d->descriptor);
    return 0;
  }

  snprintf(d->terminal_type, sizeof(d->terminal_type), "%s", "vt100");
  d->terminal_width = 80;
  d->terminal_height = 25;
  d->charset_ascii = 1;
  telnet_negotiate(d->telnet, TELNET_DO, TELNET_TELOPT_TTYPE);
  telnet_negotiate(d->telnet, TELNET_DO, TELNET_TELOPT_NAWS);
  telnet_negotiate(d->telnet, TELNET_WILL, TELNET_TELOPT_MSSP);
  telnet_negotiate(d->telnet, TELNET_WILL, telnet_charset_option);
  telnet_negotiate(d->telnet, TELNET_WILL, telnet_gmcp_option);
  return 1;
}

void telnet_destroy(DESC *d) {
  if (d->telnet != NULL)
    telnet_free(d->telnet);
  d->telnet = NULL;
}

void telnet_receive(DESC *d, const char *buffer, size_t size) {
  telnet_recv(d->telnet, buffer, size);
}

static int telnet_connected_count(void) {
  DESC *d;
  int count = 0;

  DESC_ITER_CONN(d) { count++; }
  return count;
}

static int telnet_charset_is_ascii(const char *buffer, size_t size) {
  static const char ascii[] = "US-ASCII";

  return size == sizeof(ascii) - 1 &&
         strncasecmp(buffer, ascii, sizeof(ascii) - 1) == 0;
}

static void telnet_process_data(DESC *d, const char *buffer, size_t size) {
  size_t iter;
  unsigned char current;

  for (iter = 0; iter < size; iter++) {
    current = (unsigned char)buffer[iter];
    if (current == '\n') {
      d->input_size = 0;
      if (d->flags & DS_CONNECTED) {
        run_command(d, d->input);
      } else {
        if (!do_unauth_command(d, d->input)) {
          dprintk("logout on %p fd %d, bailing.", d, d->descriptor);
          if (!(d->flags & DS_DEAD))
            shutdownsock(d, R_QUIT);
          break;
        }
      }
      memset(d->input, 0, sizeof(d->input));
      d->input_tail = 0;
      if (d->flags & DS_DEAD)
        break;
    } else if (current == '\b' || current == 0x7f) {
      if (current == 127)
        queue_string(d, "\b \b");
      else
        queue_string(d, " \b");
      if (d->input_tail > 0) {
        d->input[--d->input_tail] = '\0';
        d->input_size--;
      }
    } else if (isascii(current) && isprint(current)) {
      if ((size_t)d->input_tail >= sizeof(d->input))
        continue;
      d->input[d->input_tail++] = current;
      d->input_size++;
    }
  }
}

static void telnet_send_charset_accepted(telnet_t *telnet) {
  static const char accepted[] = {
      telnet_charset_accepted, 'U', 'S', '-', 'A', 'S', 'C', 'I', 'I'};

  telnet_subnegotiation(telnet, telnet_charset_option, accepted,
                        sizeof(accepted));
}

static void telnet_send_charset_rejected(telnet_t *telnet) {
  static const char rejected[] = {telnet_charset_rejected};

  telnet_subnegotiation(telnet, telnet_charset_option, rejected,
                        sizeof(rejected));
}

static void telnet_send_charset_request(DESC *d) {
  static const char request[] = {
      telnet_charset_request, ';', 'U', 'S', '-', 'A', 'S', 'C', 'I', 'I'};

  if (d->charset_request_pending)
    return;
  d->charset_request_pending = 1;
  telnet_subnegotiation(d->telnet, telnet_charset_option, request,
                        sizeof(request));
}

static void telnet_handle_charset(DESC *d, const char *buffer, size_t size) {
  size_t current;
  size_t start;
  char separator;

  if (size == 0)
    return;

  if (buffer[0] == telnet_charset_accepted) {
    d->charset_request_pending = 0;
    if (!telnet_charset_is_ascii(buffer + 1, size - 1)) {
      log_error(LOG_PROBLEMS, "TELNET", "CHARSET",
                "Descriptor %d accepted unsupported charset.", d->descriptor);
    }
    return;
  }
  if (buffer[0] == telnet_charset_rejected) {
    d->charset_request_pending = 0;
    return;
  }
  if (buffer[0] != telnet_charset_request || size < 3) {
    telnet_send_charset_rejected(d->telnet);
    return;
  }
  if (d->charset_request_pending) {
    telnet_send_charset_rejected(d->telnet);
    return;
  }

  separator = buffer[1];
  start = 2;
  for (current = start; current <= size; current++) {
    if (current != size && buffer[current] != separator)
      continue;
    if (telnet_charset_is_ascii(buffer + start, current - start)) {
      d->charset_ascii = 1;
      telnet_send_charset_accepted(d->telnet);
      return;
    }
    start = current + 1;
  }
  telnet_send_charset_rejected(d->telnet);
}

static void telnet_handle_gmcp(DESC *d, const char *buffer, size_t size) {
  static const char core_ping[] = "Core.Ping";
  size_t package_size = sizeof(core_ping) - 1;

  if (!d->gmcp_enabled || size < package_size ||
      memcmp(buffer, core_ping, package_size) != 0 ||
      (size > package_size && buffer[package_size] != ' '))
    return;

  telnet_send_gmcp(d->telnet, core_ping);
}

static void telnet_send_gmcp(telnet_t *telnet, const char *package) {
  telnet_subnegotiation(telnet, telnet_gmcp_option, package, strlen(package));
}

static void telnet_send_mssp_pair(telnet_t *telnet, const char *name,
                                  const char *value) {
  const char variable = TELNET_MSSP_VAR;
  const char mssp_value = TELNET_MSSP_VAL;

  telnet_send(telnet, &variable, sizeof(variable));
  telnet_send(telnet, name, strlen(name));
  telnet_send(telnet, &mssp_value, sizeof(mssp_value));
  telnet_send(telnet, value, strlen(value));
}

static void telnet_send_mssp(telnet_t *telnet) {
  char players[32];
  char uptime[32];
  char port[32];

  snprintf(players, sizeof(players), "%d", telnet_connected_count());
  snprintf(uptime, sizeof(uptime), "%lld", (long long)mudstate.start_time);
  snprintf(port, sizeof(port), "%d", mudconf.port);

  telnet_begin_sb(telnet, TELNET_TELOPT_MSSP);
  telnet_send_mssp_pair(telnet, "NAME", mudconf.mud_name);
  telnet_send_mssp_pair(telnet, "PLAYERS", players);
  telnet_send_mssp_pair(telnet, "UPTIME", uptime);
  telnet_send_mssp_pair(telnet, "CODEBASE", "BattleTechMUX");
  telnet_send_mssp_pair(telnet, "PORT", port);
  telnet_finish_sb(telnet);
}

static void telnet_event_handler(telnet_t *telnet, telnet_event_t *event,
                                 void *user_data) {
  DESC *d = user_data;
  const unsigned char *buffer;

  switch (event->type) {
  case TELNET_EV_DATA:
    telnet_process_data(d, event->data.buffer, event->data.size);
    break;
  case TELNET_EV_SEND:
    bufferevent_write(d->sock_buff, event->data.buffer, event->data.size);
    break;
  case TELNET_EV_WILL:
    if (event->neg.telopt == TELNET_TELOPT_TTYPE)
      telnet_ttype_send(telnet);
    break;
  case TELNET_EV_DO:
    if (event->neg.telopt == TELNET_TELOPT_MSSP)
      telnet_send_mssp(telnet);
    else if (event->neg.telopt == telnet_charset_option)
      telnet_send_charset_request(d);
    else if (event->neg.telopt == telnet_gmcp_option)
      d->gmcp_enabled = 1;
    break;
  case TELNET_EV_DONT:
    if (event->neg.telopt == telnet_charset_option)
      d->charset_request_pending = 0;
    else if (event->neg.telopt == telnet_gmcp_option)
      d->gmcp_enabled = 0;
    break;
  case TELNET_EV_WONT:
    if (event->neg.telopt == TELNET_TELOPT_TTYPE) {
      snprintf(d->terminal_type, sizeof(d->terminal_type), "%s", "vt100");
    } else if (event->neg.telopt == TELNET_TELOPT_NAWS) {
      d->terminal_width = 80;
      d->terminal_height = 25;
    }
    break;
  case TELNET_EV_TTYPE:
    if (event->ttype.cmd == TELNET_TTYPE_IS && event->ttype.name != NULL) {
      snprintf(d->terminal_type, sizeof(d->terminal_type), "%s",
               event->ttype.name);
    }
    break;
  case TELNET_EV_SUBNEGOTIATION:
    if (event->sub.telopt == telnet_charset_option) {
      telnet_handle_charset(d, event->sub.buffer, event->sub.size);
    } else if (event->sub.telopt == telnet_gmcp_option) {
      telnet_handle_gmcp(d, event->sub.buffer, event->sub.size);
    } else if (event->sub.telopt == TELNET_TELOPT_NAWS &&
               event->sub.size == 4) {
      buffer = (const unsigned char *)event->sub.buffer;
      d->terminal_width = (buffer[0] << 8) | buffer[1];
      d->terminal_height = (buffer[2] << 8) | buffer[3];
    }
    break;
  case TELNET_EV_WARNING:
    log_error(LOG_PROBLEMS, "TELNET", "WARN", "%s", event->error.msg);
    break;
  case TELNET_EV_ERROR:
    log_error(LOG_PROBLEMS, "TELNET", "ERROR", "%s", event->error.msg);
    shutdownsock(d, R_SOCKDIED);
    break;
  default:
    break;
  }
}
