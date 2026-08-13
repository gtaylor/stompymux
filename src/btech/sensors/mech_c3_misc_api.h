
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

Mech *mech_network_temporary_unit(BtechContext *context, int w_idx,
                                  const DbRef *network, int network_size);
Mech *mech_network_unit(Mech *mech, int w_idx, bool check_ecm,
                        bool check_started, bool t_check_uncon, bool is_c3);
void mech_network_build_temporary(Mech *mech, DbRef *network, int *network_size,
                                  bool check_ecm, bool check_started,
                                  bool t_check_uncon, bool is_c3);
void mech_network_send_message(DbRef player, Mech *mech, const char *msg,
                               bool is_c3);
void mech_network_show_targets(DbRef player, Mech *mech, bool is_c3);
void mech_network_show_status(DbRef player, Mech *mech, bool is_c3);
int mech_network_visibility(const MechNetworkVisibilityRequest *request);
float mech_network_range(Mech *mech, Mech *target, float real_range,
                         DbRef *c3_reference, bool is_c3);
float mech_network_range_with_members(Mech *mech, float real_range,
                                      Mech *target, const DbRef *network,
                                      int network_size, DbRef *c3_reference);
