/* Declares the BattleTech unit tech do API. */

#pragma once

#include "mux/server/platform.h"
#include "repair_job.h"

/* mech.tech.do.c */
int valid_ammo_mode(Mech *mech, int loc, int part, int let);
int find_ammo_type(Mech *mech, int loc, int part);
int replace_econ(const RepairOperationCall *call);
int reload_econ(const RepairOperationCall *call);
int fixarmor_econ(const RepairOperationCall *call);
int fixinternal_econ(const RepairOperationCall *call);
int repair_econ(const RepairOperationCall *call);
int repairenhcrit_econ(const RepairOperationCall *call);
int reattach_econ(const RepairOperationCall *call);
int replacesuit_econ(const RepairOperationCall *call);
int reseal_econ(const RepairOperationCall *call);
int replacep_succ(const RepairOperationCall *call);
int replaceg_succ(const RepairOperationCall *call);
int reload_succ(const RepairOperationCall *call);
int fixinternal_succ(const RepairOperationCall *call);
int fixarmor_succ(const RepairOperationCall *call);
int reattach_succ(const RepairOperationCall *call);
int replacesuit_succ(const RepairOperationCall *call);
int reseal_succ(const RepairOperationCall *call);
int repairg_succ(const RepairOperationCall *call);
int repairenhcrit_succ(const RepairOperationCall *call);
int repairp_succ(const RepairOperationCall *call);
int replacep_fail(const RepairOperationCall *call);
int repairp_fail(const RepairOperationCall *call);
int replaceg_fail(const RepairOperationCall *call);
int repairg_fail(const RepairOperationCall *call);
int repairenhcrit_fail(const RepairOperationCall *call);
int reload_fail(const RepairOperationCall *call);
int fixarmor_fail(const RepairOperationCall *call);
int fixinternal_fail(const RepairOperationCall *call);
int reattach_fail(const RepairOperationCall *call);
int replacesuit_fail(const RepairOperationCall *call);
int reseal_fail(const RepairOperationCall *call);
