#pragma once

#include <stddef.h>

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

constexpr size_t BOOTSTRAP_PASSWORD_SIZE = 33;

bool database_bootstrap_god_is_wizard_player(
    const ServerConfiguration *configuration);
int database_bootstrap(EvaluationContext *evaluation,
                       char god_password[BOOTSTRAP_PASSWORD_SIZE],
                       char wizard_password[BOOTSTRAP_PASSWORD_SIZE]);
