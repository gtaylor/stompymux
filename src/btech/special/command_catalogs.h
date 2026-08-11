#pragma once

#include <stddef.h>

#include "command_registry.h"

extern const BtechCommandDefinition MECHCOMMANDS[];
extern const BtechCommandDefinition MAPCOMMANDS[];
extern const BtechCommandDefinition MECHREPCOMMANDS[];
extern const BtechCommandDefinition AUTOPILOTCOMMANDS[];
extern const BtechCommandDefinition TURRETCOMMANDS[];
extern const BtechCommandDefinition DEBUGCOMMANDS[];
extern const BtechCommandDefinition SSCOMMANDS[];
size_t mech_command_count(void);
size_t map_command_count(void);
size_t repair_command_count(void);
size_t autopilot_command_count(void);
size_t turret_command_count(void);
size_t debug_command_count(void);
