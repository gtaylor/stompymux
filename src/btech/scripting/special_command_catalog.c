#include "command_catalogs.h"
#include "command_invokers.h"
#include "command_registry.h"

const BtechCommandDefinition SSCOMMANDS[] = {
    {0, "@SETSPECIAL <NAME> <VALUE|DATA>", "@Sets a BTech object field",
     btech_command_invoke_set_special_value},
    {0, "@VIEWSPECIAL", "@Views BTech object fields",
     btech_command_invoke_list_special_values},
    {0, nullptr, nullptr, nullptr}};
