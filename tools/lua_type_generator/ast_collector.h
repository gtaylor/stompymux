#pragma once

#include <memory>

#include "clang/Tooling/Tooling.h"

#include "model.h"

namespace lua_types {

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_collector_factory(Model &model);

} // namespace lua_types
