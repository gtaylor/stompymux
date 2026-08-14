/* connect_flow.c - Connect/create login flow for not-yet-authenticated
 * descriptors. */

#include "mux/network/connect_flow.h"

#include <ctype.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <utils.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_queue.h"
#include "mux/commands/command_runtime.h"
#include "mux/network/connection_events.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/network/network_output.h"
#include "mux/network/telnet_environment.h"
#include "mux/network/telnet_handler.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/file_cache.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/validation.h"
#include "mux/world/move.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

typedef enum ConnectResult {
  CONNECT_RESULT_CONNECTED,
  CONNECT_RESULT_RETRY,
  CONNECT_RESULT_TERMINATED,
} ConnectResult;

typedef struct ConnectFlowData {
  char name[PLAYER_NAME_LIMIT + 1];
  char password[LBUF_SIZE];
  char prompt[SBUF_SIZE];
} ConnectFlowData;

static constexpr char CONNECT_FAILURE[] =
    "Either that player does not exist, or has a different password.\r\n";
static constexpr char CREATE_FAILURE[] =
    "Either there is already a player with that name, or that name is "
    "illegal.\r\n";

constexpr int LOGIN_THROTTLE_ENTRIES = 1024;

typedef struct LoginThrottleEntry LoginThrottleEntry;
struct LoginThrottleEntry {
  char address[sizeof(((Descriptor *)0)->addr)];
  time_t last_refill;
  unsigned int tokens;
};

struct LoginThrottle {
  LoginThrottleEntry entries[LOGIN_THROTTLE_ENTRIES];
  time_t hash_window;
  int hash_count;
};

LoginThrottle *login_throttle_create(void) {
  return calloc(1, sizeof(LoginThrottle));
}

static LoginThrottleEntry *login_throttle_entry_at(LoginThrottle *throttle,
                                                   size_t index) {
  return checked_storage_at(throttle->entries, LOGIN_THROTTLE_ENTRIES,
                            sizeof(*throttle->entries), index);
}

void login_throttle_destroy(LoginThrottle *throttle) { free(throttle); }

static LoginThrottleEntry *
login_throttle_entry(LoginThrottle *throttle,
                     const ServerConfiguration *configuration,
                     const char *address, time_t now) {
  LoginThrottleEntry *oldest;
  int index;

  oldest = login_throttle_entry_at(throttle, 0);
  for (index = 0; index < LOGIN_THROTTLE_ENTRIES; index++) {
    LoginThrottleEntry *entry =
        login_throttle_entry_at(throttle, (size_t)index);

    if (!strcmp(entry->address, address))
      return entry;
    if (entry->last_refill < oldest->last_refill)
      oldest = entry;
  }
  (void)snprintf(oldest->address, sizeof(oldest->address), "%s", address);
  oldest->last_refill = now;
  oldest->tokens = (unsigned int)configuration->login_attempt_burst;
  return oldest;
}

static int login_throttle_allow(LoginThrottle *throttle,
                                const ServerConfiguration *configuration,
                                const char *address) {
  LoginThrottleEntry *entry;
  time_t now;
  time_t elapsed;
  unsigned int refills;

  if (configuration->login_attempt_burst < 1 ||
      configuration->login_attempt_refill < 1 ||
      configuration->login_hash_limit < 1) {
    return 0;
  }

  now = time(nullptr);
  if (throttle->hash_window != now) {
    throttle->hash_window = now;
    throttle->hash_count = 0;
  }
  if (throttle->hash_count >= configuration->login_hash_limit)
    return 0;

  entry = login_throttle_entry(throttle, configuration, address, now);
  elapsed = now - entry->last_refill;
  if (elapsed >= configuration->login_attempt_refill) {
    refills = (unsigned int)(elapsed / configuration->login_attempt_refill);
    if (refills >= (unsigned int)configuration->login_attempt_burst ||
        entry->tokens + refills >=
            (unsigned int)configuration->login_attempt_burst) {
      entry->tokens = (unsigned int)configuration->login_attempt_burst;
    } else {
      entry->tokens += refills;
    }
    entry->last_refill +=
        (time_t)(refills * (unsigned int)configuration->login_attempt_refill);
  }
  if (entry->tokens == 0)
    return 0;

  entry->tokens--;
  throttle->hash_count++;
  return 1;
}

/* Hide the length of a line that may contain a password from SESSION. */
static void connect_flow_hide_input_length(Descriptor *d, const char *input) {
  d->input_tot -= (int)(strlen(input) + 1);
}

typedef struct ConnectTerminationRequest {
  Descriptor *descriptor;
  const char *log_code;
  const char *log_type;
  const char *log_reason;
  DescriptorShutdownReason disconnect_reason;
  DbRef player;
  const char *user;
  int file_cache;
  const char *message;
} ConnectTerminationRequest;

static void connect_flow_terminate(const ConnectTerminationRequest *request) {
  Descriptor *d = request->descriptor;
  STARTLOG(descriptor_log(d), LOG_LOGIN | LOG_SECURITY, request->log_code,
           "RJCT") {
    char buff[MBUF_SIZE];
    (void)snprintf(buff, MBUF_SIZE, "[%d/%s] %s rejected to ", d->descriptor,
                   d->addr, request->log_type);
    log_text(buff);
    if (request->player != NOTHING)
      log_name(descriptor_log(d), request->player);
    else
      log_text(request->user);
    log_text(" (");
    log_text(request->log_reason);
    log_text(")");
    ENDLOG(descriptor_log(d));
  }
  fcache_dump(descriptor_runtime(d)->files, d, request->file_cache);
  if (request->message && *request->message) {
    descriptor_queue_string(d, request->message);
    descriptor_queue_write(d, "\r\n", 2);
  }
  descriptor_shutdown(d, request->disconnect_reason);
}

static int connect_flow_count_connected(DescriptorRegistry *registry) {
  DescriptorIterator iterator = descriptor_iterator_connected(registry);
  int count = 0;

  while (descriptor_iterator_next(&iterator) != nullptr)
    count++;
  return count;
}

static ConnectResult connect_flow_attempt_login(Descriptor *d, char *name,
                                                const char *password) {
  CommandRuntime *runtime = descriptor_runtime(d);
  ServerConfiguration *configuration = runtime->world->configuration;
  int nplayers;
  DbRef player;
  char *buff;

  nplayers = configuration->max_players < 0
                 ? configuration->max_players - 1
                 : connect_flow_count_connected(runtime->descriptors);

  if (!login_throttle_allow(runtime->login_throttle, configuration, d->addr)) {
    connect_flow_terminate(&(ConnectTerminationRequest){
        .descriptor = d,
        .log_code = "CON",
        .log_type = "Connect",
        .log_reason = "Login throttled",
        .disconnect_reason = DESCRIPTOR_SHUTDOWN_BADLOGIN,
        .player = NOTHING,
        .user = name,
        .file_cache = FC_CONN,
        .message = CONNECT_FAILURE});
    return CONNECT_RESULT_TERMINATED;
  }

  player = connect_player(&(PlayerConnectionRequest){
      .evaluation = &runtime->background_command->evaluation,
      .world = runtime->world,
      .name = name,
      .password = password,
      .host = d->addr,
      .username = d->username});
  if (player == NOTHING) {
    descriptor_queue_string(d, CONNECT_FAILURE);
    STARTLOG(descriptor_log(d), LOG_LOGIN | LOG_SECURITY, "CON", "BAD") {
      buff = alloc_lbuf("connect_flow_attempt_login.LOG.bad");
      (void)snprintf(buff, LBUF_SIZE,
                     "[%d/%s] Failed login attempt to player '%.3800s'",
                     d->descriptor, d->addr, name);
      log_text(buff);
      free_lbuf(buff);
      ENDLOG(descriptor_log(d));
    }
    if (--(d->retries_left) <= 0) {
      descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_BADLOGIN);
      return CONNECT_RESULT_TERMINATED;
    }
    return CONNECT_RESULT_RETRY;
  }

  if ((configuration->is_login_enabled &&
       nplayers < configuration->max_players) ||
      is_wizard(descriptor_runtime(d)->world->database, player) ||
      is_god(descriptor_runtime(d)->world->database, player)) {
    STARTLOG(descriptor_log(d), LOG_LOGIN, "CON", "LOGIN") {
      char log_buffer[MBUF_SIZE];
      (void)snprintf(log_buffer, sizeof(log_buffer), "[%d/%s] Connected to ",
                     d->descriptor, d->addr);
      log_text(log_buffer);
      log_name_and_loc(descriptor_log(d), player);
      ENDLOG(descriptor_log(d));
    }
    d->is_connected = true;
    d->connected_at = time(0);
    d->player = player;
    set_lastsite(d, nullptr);
    announce_connect(player, d);
    return CONNECT_RESULT_CONNECTED;
  }

  if (!configuration->is_login_enabled) {
    connect_flow_terminate(&(ConnectTerminationRequest){
        .descriptor = d,
        .log_code = "CON",
        .log_type = "Connect",
        .log_reason = "Logins Disabled",
        .disconnect_reason = DESCRIPTOR_SHUTDOWN_GAMEDOWN,
        .player = player,
        .user = name,
        .file_cache = FC_CONN_DOWN,
        .message = configuration->down_msg});
    return CONNECT_RESULT_TERMINATED;
  }

  connect_flow_terminate(&(ConnectTerminationRequest){
      .descriptor = d,
      .log_code = "CON",
      .log_type = "Connect",
      .log_reason = "Game Full",
      .disconnect_reason = DESCRIPTOR_SHUTDOWN_GAMEFULL,
      .player = player,
      .user = name,
      .file_cache = FC_CONN_FULL,
      .message = configuration->full_msg});
  return CONNECT_RESULT_TERMINATED;
}

static ConnectResult connect_flow_attempt_create(Descriptor *d, char *name,
                                                 const char *password) {
  CommandRuntime *runtime = descriptor_runtime(d);
  ServerConfiguration *configuration = runtime->world->configuration;
  int nplayers;
  DbRef player;

  if (!configuration->is_login_enabled) {
    connect_flow_terminate(&(ConnectTerminationRequest){
        .descriptor = d,
        .log_code = "CRE",
        .log_type = "Create",
        .log_reason = "Logins Disabled",
        .disconnect_reason = DESCRIPTOR_SHUTDOWN_GAMEDOWN,
        .player = NOTHING,
        .user = name,
        .file_cache = FC_CONN_DOWN,
        .message = configuration->down_msg});
    return CONNECT_RESULT_TERMINATED;
  }

  nplayers = configuration->max_players < 0
                 ? configuration->max_players
                 : connect_flow_count_connected(runtime->descriptors);
  if (nplayers > configuration->max_players) {
    connect_flow_terminate(&(ConnectTerminationRequest){
        .descriptor = d,
        .log_code = "CRE",
        .log_type = "Create",
        .log_reason = "Game Full",
        .disconnect_reason = DESCRIPTOR_SHUTDOWN_GAMEFULL,
        .player = NOTHING,
        .user = name,
        .file_cache = FC_CONN_FULL,
        .message = configuration->full_msg});
    return CONNECT_RESULT_TERMINATED;
  }

  if (!login_throttle_allow(runtime->login_throttle, configuration, d->addr)) {
    connect_flow_terminate(&(ConnectTerminationRequest){
        .descriptor = d,
        .log_code = "CRE",
        .log_type = "Create",
        .log_reason = "Login throttled",
        .disconnect_reason = DESCRIPTOR_SHUTDOWN_BADLOGIN,
        .player = NOTHING,
        .user = name,
        .file_cache = FC_CONN,
        .message = CREATE_FAILURE});
    return CONNECT_RESULT_TERMINATED;
  }

  player = create_player(&(PlayerCreationRequest){
      .evaluation = &runtime->background_command->evaluation,
      .name = name,
      .password = password});
  if (player == NOTHING) {
    descriptor_queue_string(d, CREATE_FAILURE);
    STARTLOG(descriptor_log(d), LOG_SECURITY | LOG_PCREATES, "CON", "BAD") {
      char log_buffer[MBUF_SIZE];
      (void)snprintf(log_buffer, sizeof(log_buffer),
                     "[%d/%s] Create of '%s' failed", d->descriptor, d->addr,
                     name);
      log_text(log_buffer);
      ENDLOG(descriptor_log(d));
    }
    return CONNECT_RESULT_RETRY;
  }

  STARTLOG(descriptor_log(d), LOG_LOGIN | LOG_PCREATES, "CON", "CREA") {
    char log_buffer[MBUF_SIZE];
    (void)snprintf(log_buffer, sizeof(log_buffer), "[%d/%s] Created ",
                   d->descriptor, d->addr);
    log_text(log_buffer);
    log_name(descriptor_log(d), player);
    ENDLOG(descriptor_log(d));
  }
  move_object(&descriptor_runtime(d)->background_command->evaluation, player,
              configuration->start_room);

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
  size_t length = strlen(input);
  size_t offset = 0;

  while (offset < length) {
    unsigned char character = *(const unsigned char *)checked_storage_at_const(
        input, length, sizeof(char), offset);

    if (!isascii(character) || !(isspace)(character))
      break;
    offset++;
  }
  return offset == length;
}

static FlowOutcome connect_flow_step_username(const FlowStepCall *call) {
  Descriptor *d = call->descriptor;
  ConnectFlowData *data = call->flow_data;
  const char *input = call->input;
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
  string_copy_trunc(data->name, input, sizeof(data->name) - 1);
  outcome.action = FLOW_ACTION_GOTO;
  if (lookup_player(descriptor_runtime(d)->world, NOTHING, data->name, 0) !=
      NOTHING) {
    string_copy_trunc(outcome.next_step, "password", FLOW_STEP_NAME_SIZE - 1);
  } else if (!ok_new_player_name(descriptor_runtime(d)->world->configuration,
                                 data->name)) {
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "New usernames must start with a letter and be at least "
                     "two characters long.\r\nWho are you? ";
  } else {
    string_copy_trunc(outcome.next_step, "confirm_create",
                      FLOW_STEP_NAME_SIZE - 1);
  }
  return outcome;
}

static FlowOutcome connect_flow_step_password(const FlowStepCall *call) {
  Descriptor *d = call->descriptor;
  ConnectFlowData *data = call->flow_data;
  const char *input = call->input;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    descriptor_telnet_set_echo(d, 0);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = "Password: ";
    return outcome;
  }
  descriptor_telnet_set_echo(d, 1);
  connect_flow_hide_input_length(d, input);
  string_copy_trunc(data->password, input, sizeof(data->password) - 1);

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
    string_copy_trunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
}

static FlowOutcome connect_flow_step_confirm_create(const FlowStepCall *call) {
  ConnectFlowData *data = call->flow_data;
  const char *input = call->input;
  FlowOutcome outcome = {0};

  if (input == nullptr) {
    (void)snprintf(data->prompt, sizeof(data->prompt),
                   "No character named '%s' exists. Create a new one? (Y/n) ",
                   data->name);
    outcome.action = FLOW_ACTION_WAIT;
    outcome.prompt = data->prompt;
    return outcome;
  }
  if (connect_flow_blank(input) || flow_parse_yesno(input) == FLOW_YESNO_YES) {
    outcome.action = FLOW_ACTION_GOTO;
    string_copy_trunc(outcome.next_step, "create_password",
                      FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
  if (flow_parse_yesno(input) == FLOW_YESNO_NO) {
    outcome.action = FLOW_ACTION_GOTO;
    string_copy_trunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
  outcome.action = FLOW_ACTION_WAIT;
  outcome.prompt = "Please answer y or n: ";
  return outcome;
}

static FlowOutcome connect_flow_step_create_password(const FlowStepCall *call) {
  Descriptor *d = call->descriptor;
  ConnectFlowData *data = call->flow_data;
  const char *input = call->input;
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
  string_copy_trunc(data->password, input, sizeof(data->password) - 1);
  outcome.action = FLOW_ACTION_GOTO;
  string_copy_trunc(outcome.next_step, "create_confirm_password",
                    FLOW_STEP_NAME_SIZE - 1);
  return outcome;
}

static FlowOutcome
connect_flow_step_create_confirm_password(const FlowStepCall *call) {
  Descriptor *d = call->descriptor;
  ConnectFlowData *data = call->flow_data;
  const char *input = call->input;
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
    string_copy_trunc(outcome.next_step, "create_password",
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
    outcome.prompt = CREATE_FAILURE;
    string_copy_trunc(outcome.next_step, "username", FLOW_STEP_NAME_SIZE - 1);
    return outcome;
  }
}

static FlowOutcome connect_flow_dispatch(const FlowStepCall *call) {
  const char *step = call->step;

  if (!strcmp(step, "username"))
    return connect_flow_step_username(call);
  if (!strcmp(step, "password"))
    return connect_flow_step_password(call);
  if (!strcmp(step, "confirm_create"))
    return connect_flow_step_confirm_create(call);
  if (!strcmp(step, "create_password"))
    return connect_flow_step_create_password(call);
  if (!strcmp(step, "create_confirm_password"))
    return connect_flow_step_create_confirm_password(call);

  log_error((LogEntry){.log = descriptor_log(call->descriptor),
                       .key = LOG_BUGS,
                       .primary = "FLOW",
                       .secondary = "STEP"},
            "Unknown connect flow step '%s'.", step);
  return (FlowOutcome){.action = FLOW_ACTION_CANCEL};
}

void descriptor_start_connect_flow(Descriptor *d) {
  ConnectFlowData *data = malloc(sizeof(ConnectFlowData));

  data->name[0] = '\0';
  data->password[0] = '\0';
  data->prompt[0] = '\0';
  descriptor_flow_start(&(FlowStartRequest){.descriptor = d,
                                            .initial_step = "username",
                                            .step = connect_flow_dispatch,
                                            .flow_data = data,
                                            .destroy = connect_flow_data_free});
}
