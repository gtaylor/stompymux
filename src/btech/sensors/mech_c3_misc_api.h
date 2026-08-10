
/*
   p.mech.c3.misc.h
*/

#pragma once

#include <stdbool.h>

typedef struct BtechContext BtechContext;
typedef struct Mech Mech;

#include "mux/server/platform.h"

typedef struct MechNetworkVisibilityRequest {
  Mech *observer;
  Mech *target;
  bool is_c3;
} MechNetworkVisibilityRequest;

Mech *mech_network_temporary_unit(BtechContext *context, int index,
                                  const DbRef *network, int network_size);
Mech *mech_network_unit(Mech *mech, int index, bool check_ecm,
                        bool check_started, bool check_unconscious, bool is_c3);
void mech_network_build_temporary(Mech *mech, DbRef *network, int *network_size,
                                  bool check_ecm, bool check_started,
                                  bool check_unconscious, bool is_c3);
void mech_network_send_message(DbRef player, Mech *mech, const char *message,
                               bool is_c3);
void mech_network_show_targets(DbRef player, Mech *mech, bool is_c3);
void mech_network_show_status(DbRef player, Mech *mech, bool is_c3);
int mech_network_visibility(const MechNetworkVisibilityRequest *request);
float mech_network_range(Mech *mech, Mech *target, float real_range,
                         DbRef *c3_reference, bool is_c3);
float mech_network_range_with_members(Mech *mech, Mech *target,
                                      float real_range, const DbRef *network,
                                      int network_size, DbRef *c3_reference);
void mech_network_debug(BtechContext *context, const char *message);
