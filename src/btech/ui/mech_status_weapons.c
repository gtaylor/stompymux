#include "mech_status_render_internal.h"

void PrintWeaponStatus(EvaluationContext *evaluation, Mech *mech,
                       DbRef player) {
  print_weapon_status(evaluation, mech, player, false, nullptr, 0);
}
