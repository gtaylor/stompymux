/* connect_flow.c - Connect/create login flow for not-yet-authenticated
 * descriptors. */

#include "mux/network/connect_flow.h"

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_handler.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/file_cache.h"
#include "mux/server/log.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/password.h"
#include "mux/support/stringutil.h"
#include "mux/world/move.h"
#include "mux/world/player.h"

typedef enum ConnectResult {
  CONNECT_RESULT_CONNECTED,
  CONNECT_RESULT_RETRY,
  CONNECT_RESULT_TERMINATED,
} ConnectResult;

typedef struct ConnectFlowData {
  char name[PLAYER_NAME_LIMIT + 1];
  char password[LBUF_SIZE];
} ConnectFlowData;

static const char *connect_fail =
    "Either that player does not exist, or has a different password.\r\n";
static const char *create_fail = "Either there is already a player with that "
                                 "name, or that name is illegal.\r\n";

constexpr int LOGIN_THROTTLE_ENTRIES = 1024;

typedef struct LoginThrottleEntry LoginThrottleEntry;
struct LoginThrottleEntry {
  char address[sizeof(((Descriptor *)0)->addr)];
  time_t last_refill;
  unsigned int tokens;
};

static LoginThrottleEntry login_throttle_entries[LOGIN_THROTTLE_ENTRIES];
static time_t login_hash_window;
static int login_hash_count;

static LoginThrottleEntry *login_throttle_entry(const char *address,
                                                time_t now) {
  LoginThrottleEntry *oldest;
  int index;

  oldest = &login_throttle_entries[0];
  for (index = 0; index < LOGIN_THROTTLE_ENTRIES; index++) {
    if (!strcmp(login_throttle_entries[index].address, address))
      return &login_throttle_entries[index];
    if (login_throttle_entries[index].last_refill < oldest->last_refill)
      oldest = &login_throttle_entries[index];
  }
  snprintf(oldest->address, sizeof(oldest->address), "%s", address);
  oldest->last_refill = now;
  oldest->tokens = (unsigned int)mudconf.login_attempt_burst;
  return oldest;
}

static int login_throttle_allow(const char *address) {
  LoginThrottleEntry *entry;
  time_t now;
  time_t elapsed;
  unsigned int refills;

  if (mudconf.login_attempt_burst < 1 || mudconf.login_attempt_refill < 1 ||
      mudconf.login_hash_limit < 1) {
    return 0;
  }

  now = time(nullptr);
  if (login_hash_window != now) {
    login_hash_window = now;
    login_hash_count = 0;
  }
  if (login_hash_count >= mudconf.login_hash_limit)
    return 0;

  entry = login_throttle_entry(address, now);
  elapsed = now - entry->last_refill;
  if (elapsed >= mudconf.login_attempt_refill) {
    refills = (unsigned int)(elapsed / mudconf.login_attempt_refill);
    if (refills >= (unsigned int)mudconf.login_attempt_burst ||
        entry->tokens + refills >= (unsigned int)mudconf.login_attempt_burst) {
      entry->tokens = (unsigned int)mudconf.login_attempt_burst;
    } else {
      entry->tokens += refills;
    }
    entry->last_refill += refills * (unsigned int)mudconf.login_attempt_refill;
  }
  if (entry->tokens == 0)
    return 0;

  entry->tokens--;
  login_hash_count++;
  return 1;
}

/* Hide the length of a line that may contain a password from SESSION. */
static void connect_flow_hide_input_length(Descriptor *d, const char *input) {
  d->input_tot -= (int)(strlen(input) + 1);
}

static void connect_flow_terminate(Descriptor *d, const char *logcode,
                                   const char *logtype, const char *logreason,
                                   DescriptorShutdownReason disconnect_reason,
                                   DbRef player, const char *user,
                                   int filecache, const char *message) {
  STARTLOG(LOG_LOGIN | LOG_SECURITY, logcode, "RJCT") {
    char *buff = alloc_mbuf("connect_flow_terminate.LOG");
    snprintf(buff, MBUF_SIZE, "[%d/%s] %s rejected to ", d->descriptor, d->addr,
             logtype);
    log_text(buff);
    free_mbuf(buff);
    if (player != NOTHING)
      log_name(player);
    else
      log_text(user);
    log_text(" (");
    log_text(logreason);
    log_text(")");
    ENDLOG;
  }
  fcache_dump(d, filecache);
  if (message && *message) {
    descriptor_queue_string(d, message);
    descriptor_queue_write(d, "\r\n", 2);
  }
  descriptor_shutdown(d, disconnect_reason);
}

static int connect_flow_count_connected(void) {
  Descriptor *d;
  int count = 0;

  for (d = descriptor_first_connected(); d != nullptr;
       d = descriptor_next_connected(d))
    count++;
  return count;
}

static ConnectResult connect_flow_attempt_login(Descriptor *d, char *name,
                                                char *password) {
  int nplayers;
  DbRef player;
  char *buff;

  nplayers = mudconf.max_players < 0 ? mudconf.max_players - 1
                                     : connect_flow_count_connected();

  if (!login_throttle_allow(d->addr)) {
    connect_flow_terminate(d, "CON", "Connect", "Login throttled",
                           DESCRIPTOR_SHUTDOWN_BADLOGIN, NOTHING, name, FC_CONN,
                           connect_fail);
    return CONNECT_RESULT_TERMINATED;
  }

  player = connect_player(name, password, d->addr, d->username);
  if (player == NOTHING) {
    descriptor_queue_string(d, connect_fail);
    STARTLOG(LOG_LOGIN | LOG_SECURITY, "CON", "BAD") {
      buff = alloc_lbuf("connect_flow_attempt_login.LOG.bad");
      snprintf(buff, LBUF_SIZE,
               "[%d/%s] Failed login attempt to player '%.3800s'",
               d->descriptor, d->addr, name);
      log_text(buff);
      free_lbuf(buff);
      ENDLOG;
    }
    if (--(d->retries_left) <= 0) {
      descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_BADLOGIN);
      return CONNECT_RESULT_TERMINATED;
    }
    return CONNECT_RESULT_RETRY;
  }

  if (((mudconf.control_flags & CF_LOGIN) &&
       (nplayers < mudconf.max_players)) ||
      is_wizard(player) || is_god(player)) {
    STARTLOG(LOG_LOGIN, "CON", "LOGIN") {
      buff = alloc_mbuf("connect_flow_attempt_login.LOG.login");
      snprintf(buff, MBUF_SIZE, "[%d/%s] Connected to ", d->descriptor,
               d->addr);
      log_text(buff);
      log_name_and_loc(player);
      free_mbuf(buff);
      ENDLOG;
    }
    d->is_connected = true;
    d->connected_at = time(0);
    d->player = player;
    set_lastsite(d, nullptr);
    announce_connect(player, d);
    return CONNECT_RESULT_CONNECTED;
  }

  if (!(mudconf.control_flags & CF_LOGIN)) {
    connect_flow_terminate(d, "CON", "Connect", "Logins Disabled",
                           DESCRIPTOR_SHUTDOWN_GAMEDOWN, player, name,
                           FC_CONN_DOWN, mudconf.down_msg);
    return CONNECT_RESULT_TERMINATED;
  }

  connect_flow_terminate(d, "CON", "Connect", "Game Full",
                         DESCRIPTOR_SHUTDOWN_GAMEFULL, player, name,
                         FC_CONN_FULL, mudconf.full_msg);
  return CONNECT_RESULT_TERMINATED;
}

static ConnectResult connect_flow_attempt_create(Descriptor *d, char *name,
                                                 char *password) {
  int nplayers;
  DbRef player;
  char *buff;

  if (!(mudconf.control_flags & CF_LOGIN)) {
    connect_flow_terminate(d, "CRE", "Create", "Logins Disabled",
                           DESCRIPTOR_SHUTDOWN_GAMEDOWN, NOTHING, name,
                           FC_CONN_DOWN, mudconf.down_msg);
    return CONNECT_RESULT_TERMINATED;
  }

  nplayers = mudconf.max_players < 0 ? mudconf.max_players
                                     : connect_flow_count_connected();
  if (nplayers > mudconf.max_players) {
    connect_flow_terminate(d, "CRE", "Create", "Game Full",
                           DESCRIPTOR_SHUTDOWN_GAMEFULL, NOTHING, name,
                           FC_CONN_FULL, mudconf.full_msg);
    return CONNECT_RESULT_TERMINATED;
  }

  if (!login_throttle_allow(d->addr)) {
    connect_flow_terminate(d, "CRE", "Create", "Login throttled",
                           DESCRIPTOR_SHUTDOWN_BADLOGIN, NOTHING, name, FC_CONN,
                           create_fail);
    return CONNECT_RESULT_TERMINATED;
  }

  player = create_player(name, password, NOTHING, 0);
  if (player == NOTHING) {
    descriptor_queue_string(d, create_fail);
    STARTLOG(LOG_SECURITY | LOG_PCREATES, "CON", "BAD") {
      buff = alloc_mbuf("connect_flow_attempt_create.LOG.badcrea");
      snprintf(buff, MBUF_SIZE, "[%d/%s] Create of '%s' failed", d->descriptor,
               d->addr, name);
      log_text(buff);
      free_mbuf(buff);
      ENDLOG;
    }
    return CONNECT_RESULT_RETRY;
  }

  STARTLOG(LOG_LOGIN | LOG_PCREATES, "CON", "CREA") {
    buff = alloc_mbuf("connect_flow_attempt_create.LOG.create");
    snprintf(buff, MBUF_SIZE, "[%d/%s] Created ", d->descriptor, d->addr);
    log_text(buff);
    log_name(player);
    free_mbuf(buff);
    ENDLOG;
  }
  move_object(player, mudconf.start_room);

  d->is_connected = true;
  d->connected_at = time(0);
  d->player = player;
  set_lastsite(d, nullptr);
  announce_connect(player, d);
  return CONNECT_RESULT_CONNECTED;
}

static void connect_flow_data_free(void *flow_data) {
  ConnectFlowData *data = flow_data;

  sodium_memzero(data->password, sizeof(data->password));
  free(data);
}

static int connect_flow_blank(const char *input) {
  while (*input && isascii((unsigned char)*input) &&
         isspace((unsigned char)*input))
    input++;
  return *input == '\0';
}

static FlowOutcome connect_flow_step_username(Descriptor *d, void *flow_data,
                                              const char *step,
                                              const char *input) {
  ConnectFlowData *data = flow_data;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Who are you? ";
    return outcome;
  }
  if (connect_flow_blank(input)) {
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Who are you? ";
    return outcome;
  }
  StringCopyTrunc(data->name, input, sizeof(data->name) - 1);
  outcome.action = FLOW_ACTION_GOTO;
  if (lookup_player(NOTHING, data->name, 0) != NOTHING) {
    StringCopyTrunc(outcome.next_step, "password", FLOW_STEP_NAME_SIZE - 1);
  } else {
    StringCopyTrunc(outcome.next_step, "confirm_create",
                    FLOW_STEP_NAME_SIZE - 1);
  }
  return outcome;
}

static FlowOutcome connect_flow_step_password(Descriptor *d, void *flow_data,
                                              const char *step,
                                              const char *input) {
  ConnectFlowData *data = flow_data;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    descriptor_telnet_set_echo(d, 0);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Password: ";
    return outcome;
  }
  descriptor_telnet_set_echo(d, 1);
  connect_flow_hide_input_length(d, input);
  StringCopyTrunc(data->password, input, sizeof(data->password) - 1);

  switch (connect_flow_attempt_login(d, data->name, data->password)) {
  case CONNECT_RESULT_CONNECTED:
    outcome.action = FLOW_ACTION_DONE;
    return outcome;
  case CONNECT_RESULT_TERMINATED:
    outcome.action = FLOW_ACTION_CANCEL;
    return outcome;
  case CONNECT_RESULT_RETRY:
  default:
    outcome.action = FLOW_ACTION_GOTO;
    StringCopyTrunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
}

static FlowOutcome connect_flow_step_confirm_create(Descriptor *d,
                                                    void *flow_data,
                                                    const char *step,
                                                    const char *input) {
  ConnectFlowData *data = flow_data;
  FlowOutcome outcome = {0};
  static char prompt[SBUF_SIZE];

  if (input == nullptr) {
    snprintf(prompt, sizeof(prompt),
             "No character named '%s' exists. Create a new one? (Y/n) ",
             data->name);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = prompt;
    return outcome;
  }
  if (connect_flow_blank(input) || flow_parse_yesno(input) == FLOW_YESNO_YES) {
    outcome.action = FLOW_ACTION_GOTO;
    StringCopyTrunc(outcome.next_step, "create_password",
                    FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
  if (flow_parse_yesno(input) == FLOW_YESNO_NO) {
    outcome.action = FLOW_ACTION_GOTO;
    StringCopyTrunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
  outcome.action = FLOW_ACTION_WAIT;
  outcome.prompt = "Please answer y or n: ";
  return outcome;
}

static FlowOutcome connect_flow_step_create_password(Descriptor *d,
                                                     void *flow_data,
                                                     const char *step,
                                                     const char *input) {
  ConnectFlowData *data = flow_data;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    /* Echo is already suppressed if we got here via a retype mismatch;
     * harmless to (re)request it either way. Left on through
     * create_confirm_password - only restored once we're done with both
     * password prompts, to avoid toggling it on and off faster than the
     * client can acknowledge each request. */
    descriptor_telnet_set_echo(d, 0);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Choose a password: ";
    return outcome;
  }
  connect_flow_hide_input_length(d, input);
  StringCopyTrunc(data->password, input, sizeof(data->password) - 1);
  outcome.action = FLOW_ACTION_GOTO;
  StringCopyTrunc(outcome.next_step, "create_confirm_password",
                  FLOW_STEP_NAME_SIZE - 1);
  return outcome;
}

static FlowOutcome
connect_flow_step_create_confirm_password(Descriptor *d, void *flow_data,
                                          const char *step, const char *input) {
  ConnectFlowData *data = flow_data;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    descriptor_telnet_set_echo(d, 0);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Retype password: ";
    return outcome;
  }
  connect_flow_hide_input_length(d, input);
  if (strcmp(input, data->password) != 0) {
    /* Still headed for another password prompt either way; leave echo
     * suppressed rather than restoring and immediately re-suppressing it. */
    outcome.action = FLOW_ACTION_GOTO;
    outcome.prompt = "Passwords did not match.\r\n";
    StringCopyTrunc(outcome.next_step, "create_password",
                    FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }

  /* Done with hidden input either way past this point (connecting,
   * disconnecting, or back to a visible username prompt). */
  descriptor_telnet_set_echo(d, 1);
  switch (connect_flow_attempt_create(d, data->name, data->password)) {
  case CONNECT_RESULT_CONNECTED:
    outcome.action = FLOW_ACTION_DONE;
    return outcome;
  case CONNECT_RESULT_TERMINATED:
    outcome.action = FLOW_ACTION_CANCEL;
    return outcome;
  case CONNECT_RESULT_RETRY:
  default:
    outcome.action = FLOW_ACTION_GOTO;
    outcome.prompt = create_fail;
    StringCopyTrunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
}

static FlowOutcome connect_flow_dispatch(Descriptor *d, void *flow_data,
                                         const char *step, const char *input) {
  if (!strcmp(step, "username"))
    return connect_flow_step_username(d, flow_data, step, input);
  if (!strcmp(step, "password"))
    return connect_flow_step_password(d, flow_data, step, input);
  if (!strcmp(step, "confirm_create"))
    return connect_flow_step_confirm_create(d, flow_data, step, input);
  if (!strcmp(step, "create_password"))
    return connect_flow_step_create_password(d, flow_data, step, input);
  if (!strcmp(step, "create_confirm_password"))
    return connect_flow_step_create_confirm_password(d, flow_data, step, input);

  log_error(LOG_BUGS, "FLOW", "STEP", "Unknown connect flow step '%s'.", step);
  return (FlowOutcome){.action = FLOW_ACTION_CANCEL};
}

void descriptor_start_connect_flow(Descriptor *d) {
  ConnectFlowData *data = malloc(sizeof(ConnectFlowData));

  data->name[0] = '\0';
  data->password[0] = '\0';
  descriptor_flow_start(d, "username", connect_flow_dispatch, data,
                        connect_flow_data_free);
}
