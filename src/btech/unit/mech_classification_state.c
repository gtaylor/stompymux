#include "mech_classification_api.h"

#include "mech_internal.h"

int mech_class(const Mech *mech) { return mech->ud.type; }
