#include "mech_internal.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

#include <stdarg.h>

static int callback_count;
static MultiWeaponSelectionCall calls[2];

EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return nullptr;
}

static bool selection_callback(const MultiWeaponSelectionCall *call) {
  if (callback_count >= 2)
    return true;
  MultiWeaponSelectionCall *slot =
      checked_storage_at(calls, 2, sizeof(*calls), (size_t)callback_count);
  *slot = *call;
  callback_count++;
  return false;
}

WeaponNumberLookupResult
weapon_number_find(const WeaponNumberLookupRequest *request [[maybe_unused]]) {
  return (WeaponNumberLookupResult){.found = true};
}

void mecha_notifyf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {
  va_list arguments;
  va_start(arguments, format);
  va_end(arguments);
}

int main(void) {
  Mech mech = {};
  char selection[] = "1-2,3";
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = &mech,
      .actor = 1,
      .selection = selection,
      .mode = 2,
      .callback = selection_callback,
  });

  return callback_count == 2 && calls[0].first == 1 && calls[0].last == 2 &&
                 calls[1].first == 3 && calls[1].last == 3
             ? 0
             : 1;
}
