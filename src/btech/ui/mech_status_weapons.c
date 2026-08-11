#include "mech_api_types.h"
#include "mech_status_api.h"
#include "mech_status_render_internal.h"
#include "mux/server/platform.h"

void print_weapon_status_summary(EvaluationContext *evaluation, Mech *mech,
                                 DbRef player) {
  print_weapon_status(evaluation, mech, player, false, nullptr, 0);
}
