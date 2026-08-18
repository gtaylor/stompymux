/** @file
 * Public MUX server interface for database bootstrap.
 */
#pragma once

#include <stddef.h>

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

constexpr size_t BOOTSTRAP_PASSWORD_SIZE = 33;

/** Executes database bootstrap god is wizard player. @param[in] configuration
 * Server configuration. */

bool database_bootstrap_god_is_wizard_player(
    const ServerConfiguration *configuration);
/** Executes database bootstrap. @param[in,out] evaluation Expression evaluation
 * context. @param[in,out] god_password God password. @param[in,out]
 * wizard_password Wizard password. */

int database_bootstrap(EvaluationContext *evaluation,
                       char god_password[BOOTSTRAP_PASSWORD_SIZE],
                       char wizard_password[BOOTSTRAP_PASSWORD_SIZE]);
