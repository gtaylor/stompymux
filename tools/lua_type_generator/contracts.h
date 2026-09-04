#pragma once

#include <filesystem>
#include <map>
#include <string>
#include <string_view>

#include "model.h"

namespace lua_types {

void parse_contract_comment(Model &model, std::string_view raw,
                            const std::string &owner, const Location &location);

void validate_model(Model &model);

std::map<std::string, std::string> render_modules(Model &model);

bool check_outputs(Model &model,
                   const std::map<std::string, std::string> &outputs,
                   const std::filesystem::path &output_directory);

bool write_outputs(Model &model,
                   const std::map<std::string, std::string> &outputs,
                   const std::filesystem::path &output_directory);

void print_diagnostics(Model &model);

} // namespace lua_types
