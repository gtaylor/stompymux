/* Structural validation for persisted and displayed repair event payloads. */

#pragma once

#include "repair_job.h"

bool repair_event_payload_structurally_valid(int event_type,
                                             RepairEventPayload payload,
                                             bool fake);
