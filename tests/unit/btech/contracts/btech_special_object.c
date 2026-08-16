#include "special_object.h"

#include <assert.h>

int main(void) {
  struct {
    BtechSpecialObject xcode;
    int payload;
  } mech = {.xcode = {.type = GTYPE_MECH}, .payload = 1};
  struct {
    BtechSpecialObject xcode;
    int payload;
  } map = {.xcode = {.type = GTYPE_MAP}, .payload = 2};

  assert(btech_special_object_as_mech(&mech.xcode) == (Mech *)&mech);
  assert(btech_special_object_as_mech(&map.xcode) == nullptr);
  assert(btech_special_object_as_mech(nullptr) == nullptr);

  assert(btech_special_object_as_map(&map.xcode) == (BattleMap *)&map);
  assert(btech_special_object_as_map(&mech.xcode) == nullptr);
  assert(btech_special_object_as_map(nullptr) == nullptr);
  return 0;
}
