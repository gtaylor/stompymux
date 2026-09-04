#pragma once

#include <cstddef>
#include <filesystem>
#include <string>
#include <string_view>
#include <vector>

#include "clang/Tooling/CompilationDatabase.h"

#include "model.h"

namespace lua_types {

bool is_project_source(std::string_view path);

bool is_mux_lua_source(std::string_view path);

bool is_mux_registration_source(std::string_view path);

bool is_btech_binding_source(std::string_view path);

std::string contract_marker_identity(std::string_view path, size_t offset);

std::vector<std::string>
select_source_files(Model &model, const std::filesystem::path &repository_root,
                    const std::vector<std::string> &candidates,
                    const clang::tooling::CompilationDatabase &compilations);

} // namespace lua_types
