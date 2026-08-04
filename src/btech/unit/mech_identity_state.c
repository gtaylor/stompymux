#include "mech_identity_api.h"

#include "mech_internal.h"

BtechContext *mech_context(const Mech *mech) { return mech->xcode.context; }

DbRef mech_dbref(const Mech *mech) { return mech->mynum; }

DbRef mech_map_dbref(const Mech *mech) { return mech->mapindex; }

int mech_map_slot(const Mech *mech) { return mech->mapnumber; }

int mech_brief_mode(const Mech *mech) { return mech->brief; }

MechUnitId mech_unit_id(const Mech *mech) {
  return (MechUnitId){.first = mech->ID[0], .second = mech->ID[1]};
}

void mech_map_dbref_set(Mech *mech, DbRef map_dbref) {
  mech->mapindex = map_dbref;
}

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
