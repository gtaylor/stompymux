/* commac_persistence.h -- SQLite commac, comsys, and macro persistence */

#pragma once

typedef struct PersistenceContext PersistenceContext;

/* Register the authoritative commac/comsys/macro SQLite extension. */
int commac_persistence_register(PersistenceContext *context);
