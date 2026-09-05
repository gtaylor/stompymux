#include "btech/commands/btech_api.h"

#undef NDEBUG
#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "btech/repair/mechrep_api.h"
#include "btech/special/registry_api.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_keys.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"

static Mech *const TARGET = (Mech *)1;
static int notification_count;
static char last_notification[128];
static void *handled_target;
static char *handled_arguments;

void raw_notify(EvaluationContext *evaluation [[maybe_unused]],
                DbRef player [[maybe_unused]], const char *message) {
  notification_count++;
  (void)snprintf(last_notification, sizeof(last_notification), "%s", message);
}

Mech *btech_context_get_mech(BtechContext *context [[maybe_unused]],
                             DbRef object) {
  return object == 2 ? TARGET : nullptr;
}

#define STUB_MECH_ADMIN_HANDLER(name)                                          \
  void name(DbRef player [[maybe_unused]], void *data [[maybe_unused]],        \
            char *arguments [[maybe_unused]]) {}

STUB_MECH_ADMIN_HANDLER(mechrep_rloadnew)
STUB_MECH_ADMIN_HANDLER(mechrep_rrestore)
STUB_MECH_ADMIN_HANDLER(mechrep_rsavetemp2)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetarmor)
STUB_MECH_ADMIN_HANDLER(mechrep_raddweap)
STUB_MECH_ADMIN_HANDLER(mechrep_rresetcrits)
STUB_MECH_ADMIN_HANDLER(mechrep_rrepair)
STUB_MECH_ADMIN_HANDLER(mechrep_rreload)
STUB_MECH_ADMIN_HANDLER(mechrep_rrestock)
STUB_MECH_ADMIN_HANDLER(mechrep_rfiremode)
STUB_MECH_ADMIN_HANDLER(mechrep_raddspecial)
STUB_MECH_ADMIN_HANDLER(mechrep_rdisplaysection)
STUB_MECH_ADMIN_HANDLER(mechrep_rshowtech)
STUB_MECH_ADMIN_HANDLER(mechrep_raddtech)
STUB_MECH_ADMIN_HANDLER(mechrep_rdeltech)
STUB_MECH_ADMIN_HANDLER(mechrep_raddinftech)
STUB_MECH_ADMIN_HANDLER(mechrep_rdelinftech)
STUB_MECH_ADMIN_HANDLER(mechrep_rsettons)
STUB_MECH_ADMIN_HANDLER(mechrep_rsettype)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetmove)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetheatsinks)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetjumpspeed)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetlrsrange)
STUB_MECH_ADMIN_HANDLER(mechrep_rsettacrange)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetscanrange)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetradio)
STUB_MECH_ADMIN_HANDLER(mechrep_rsetradiorange)
STUB_MECH_ADMIN_HANDLER(mechrep_setcargospace)

void mechrep_rsetspeed(DbRef player [[maybe_unused]], void *data,
                       char *arguments) {
  handled_target = data;
  handled_arguments = arguments;
}

int main(void) {
  GameObject objects[5] = {};
  GameDatabase database = {.object_storage = objects, .size = 4};
  WorldContext world = {.database = &database};
  CommandContext context = {.world = &world};
  context.evaluation.btech = (BtechContext *)3;
  objects[2].location = 2;

  CommandInvocation invocation = {
      .context = &context, .player = 1, .key = MECH_ADMIN_SETMAXSPEED};
  char arguments[] = "6";
  invocation.first = arguments;
  do_mech_admin(&invocation);
  assert(handled_target == TARGET);
  assert(handled_arguments == arguments);

  objects[2].location = 3;
  handled_target = nullptr;
  do_mech_admin(&invocation);
  assert(handled_target == nullptr);
  assert(strstr(last_notification, "inside a BattleTech unit") != nullptr);

  invocation.key = 0;
  notification_count = 0;
  do_mech_admin(&invocation);
  assert(notification_count == 30);
  assert(strstr(last_notification, "/setcargospace") != nullptr);
  return 0;
}
