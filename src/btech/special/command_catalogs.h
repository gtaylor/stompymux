#pragma once

#include <stddef.h>

#include "command_registry.h"

extern const BtechCommandDefinition mechcommands[];
extern const BtechCommandDefinition mapcommands[];
extern const BtechCommandDefinition mechrepcommands[];
extern const BtechCommandDefinition autopilotcommands[];
extern const BtechCommandDefinition turretcommands[];
extern const BtechCommandDefinition debugcommands[];
extern const BtechCommandDefinition sscommands[];
size_t mech_command_count(void);
size_t map_command_count(void);
size_t repair_command_count(void);
size_t autopilot_command_count(void);
size_t turret_command_count(void);
size_t debug_command_count(void);
