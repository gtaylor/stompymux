#include "command_invokers.h"
#include "command_registry.h"

const BtechCommandDefinition sscommands[] = {
    {0, "@SETXCODE <NAME> <VALUE|DATA>", "@Sets xcode value on object",
     btech_command_invoke_set_xcodestuff},
    {0, "@VIEWXCODE", "@Views xcode values on object",
     btech_command_invoke_list_xcodestuff},
    {0, NULL, NULL, NULL}};
