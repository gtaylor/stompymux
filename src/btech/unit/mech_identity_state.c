#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"

#include <string.h>

#include "mech_internal.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

BtechContext *mech_context(const Mech *mech) { return mech->xcode.context; }

DbRef mech_dbref(const Mech *mech) { return mech->mynum; }

DbRef mech_turret_dbref(const Mech *mech, int turret) {
  const DbRef *turret_dbref = checked_storage_at_const(
      mech->pd.turret, NUM_TURRETS, sizeof(*mech->pd.turret), (size_t)turret);
  return *turret_dbref;
}

DbRef mech_map_dbref(const Mech *mech) { return mech->mapindex; }

int mech_map_slot(const Mech *mech) { return mech->mapnumber; }

int mech_brief_mode(const Mech *mech) { return mech->brief; }

MechUnitId mech_unit_id(const Mech *mech) {
  return (MechUnitId){.first = mech->id[0], .second = mech->id[1]};
}

const char *mech_model_name(const Mech *mech) { return mech->ud.mech_name; }

const char *mech_model_reference(const Mech *mech) {
  return mech->ud.mech_type;
}

void mech_model_reference_set(Mech *mech, const char *reference) {
  (void)string_copy_bounded(mech->ud.mech_type, sizeof(mech->ud.mech_type),
                            reference);
}

void mech_map_dbref_set(Mech *mech, DbRef map_dbref) {
  mech->mapindex = map_dbref;
}

void mech_map_slot_set(Mech *mech, int map_slot) { mech->mapnumber = map_slot; }

void mech_brief_mode_set(Mech *mech, int mode) { mech->brief = (char)mode; }

void mech_unit_id_set(Mech *mech, char first, char second) {
  mech->id[0] = first;
  mech->id[1] = second;
}

void mech_identity_initialize(Mech *mech, DbRef dbref) {
  mech->mynum = dbref;
  mech->mapnumber = 1;
  mech->mapindex = NOTHING;
  mech_unit_id_set(mech, ' ', ' ');
}
