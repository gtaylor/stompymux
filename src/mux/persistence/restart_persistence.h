/* restart_persistence.h -- transient SQLite state for controlled restarts */

#pragma once

#include <sqlite3.h>

/* Create the restart tables in a complete game-database snapshot. */
int restart_persistence_create_schema(sqlite3 *sqlite);

/* Save live descriptors and command queues before an internal re-exec. */
int restart_persistence_store(void);

/* Restore and consume a restart payload after an internal re-exec. */
int restart_persistence_load(void);

/* Discard an unrequested, stale restart payload during an ordinary launch. */
int restart_persistence_discard(void);
