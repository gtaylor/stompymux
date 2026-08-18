/** @file
 * - SQLite commac, comsys, and macro persistence.
 */
#pragma once

typedef struct PersistenceContext PersistenceContext;

/* Register the authoritative commac/comsys/macro SQLite extension. */
/** Executes commac persistence register. @param[in,out] context Operation
 * context. */

int commac_persistence_register(PersistenceContext *context);
