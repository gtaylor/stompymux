#include "mech_identity_api.h"

#include <string.h>

#include "mech_internal.h"
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
  return (MechUnitId){.first = mech->ID[0], .second = mech->ID[1]};
}

const char *mech_model_name(const Mech *mech) { return mech->ud.mech_name; }

const char *mech_model_reference(const Mech *mech) {
  return mech->ud.mech_type;
}

void mech_model_reference_set(Mech *mech, const char *reference) {
  strncpy(mech->ud.mech_type, reference, sizeof(mech->ud.mech_type) - 1);
  mech->ud.mech_type[sizeof(mech->ud.mech_type) - 1] = '\0';
}

void mech_map_dbref_set(Mech *mech, DbRef map_dbref) {
  mech->mapindex = map_dbref;
}

void mech_map_slot_set(Mech *mech, int map_slot) { mech->mapnumber = map_slot; }

void mech_brief_mode_set(Mech *mech, int mode) { mech->brief = (char)mode; }

void mech_unit_id_set(Mech *mech, char first, char second) {
  mech->ID[0] = first;
  mech->ID[1] = second;
}

void mech_identity_initialize(Mech *mech, DbRef dbref) {
  mech->mynum = dbref;
  mech->mapnumber = 1;
  mech->mapindex = NOTHING;
  mech_unit_id_set(mech, ' ', ' ');
}
