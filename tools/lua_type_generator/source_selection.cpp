#include "source_selection.h"

#include <array>
#include <fstream>
#include <iterator>
#include <optional>
#include <set>
#include <string>
#include <system_error>
#include <utility>

namespace lua_types {
namespace {

constexpr std::string_view CONTRACT_MARKER = "@par LuaLS";

// These are the declaration and call spellings interpreted by the AST
// collector outside the always-scanned binding directories. False positives
// only parse an extra translation unit; omitting a spelling could hide a
// native catalog or registration from model validation.
constexpr std::array<std::string_view, 13> SOURCE_TRIGGERS = {
    CONTRACT_MARKER,
    "BtechLuaEntry",
    "lua_btech_install_bindings",
    "LUA_ERROR_CODE_NAMES",
    "LUA_LOCK_DEFINITIONS",
    "LUA_COMMAND_ACCESS_ENTRIES",
    "FLAG_ENTRIES",
    "POWER_ENTRIES",
    "CHANNEL_FLAGS",
    "lua_mux_object_type_namespace_index",
    "LuaMuxChannelMethod",
    "lua_pushcclosure",
    "lua_pushcfunction",
};

std::string repository_path(const std::filesystem::path &repository_root,
                            const std::filesystem::path &path) {
  std::filesystem::path absolute = path;
  if (absolute.is_relative())
    absolute = repository_root / absolute;
  absolute = absolute.lexically_normal();
  std::error_code error;
  const std::filesystem::path relative =
      std::filesystem::relative(absolute, repository_root, error);
  if (!error && !relative.empty() && !relative.string().starts_with(".."))
    return relative.generic_string();
  return absolute.generic_string();
}

std::string
compile_command_path(const clang::tooling::CompileCommand &command) {
  std::filesystem::path path(command.Filename);
  if (path.is_relative())
    path = std::filesystem::path(command.Directory) / path;
  std::error_code canonical_error;
  const std::filesystem::path canonical =
      std::filesystem::weakly_canonical(path, canonical_error);
  return canonical_error ? path.lexically_normal().generic_string()
                         : canonical.generic_string();
}

std::optional<std::string> read_binary(Model &model,
                                       const std::filesystem::path &path,
                                       const std::string &display_path) {
  std::ifstream input(path, std::ios::binary);
  if (!input.is_open()) {
    model.diagnose({.path = display_path},
                   "cannot read Lua type generator source candidate");
    return std::nullopt;
  }
  std::string contents{std::istreambuf_iterator<char>(input),
                       std::istreambuf_iterator<char>()};
  if (input.bad()) {
    model.diagnose({.path = display_path},
                   "cannot read complete Lua type generator source candidate");
    return std::nullopt;
  }
  return contents;
}

Location marker_location(const std::string &path, std::string_view contents,
                         size_t offset) {
  unsigned line = 1;
  unsigned column = 1;
  for (size_t index = 0; index < offset; index++) {
    if (contents[index] == '\n') {
      line++;
      column = 1;
    } else {
      column++;
    }
  }
  return {.path = path, .line = line, .column = column};
}

void collect_contract_markers(Model &model, const std::string &path,
                              std::string_view contents) {
  size_t offset = contents.find(CONTRACT_MARKER);
  while (offset != std::string_view::npos) {
    model.expected_contract_markers.emplace(
        contract_marker_identity(path, offset),
        marker_location(path, contents, offset));
    offset = contents.find(CONTRACT_MARKER, offset + CONTRACT_MARKER.size());
  }
}

bool has_source_trigger(std::string_view contents) {
  for (const std::string_view trigger : SOURCE_TRIGGERS) {
    if (contents.find(trigger) != std::string_view::npos)
      return true;
  }
  return false;
}

void scan_contract_headers(Model &model,
                           const std::filesystem::path &repository_root) {
  for (const std::filesystem::path relative_root : {"src/mux", "src/btech"}) {
    const std::filesystem::path root = repository_root / relative_root;
    std::error_code iterator_error;
    std::filesystem::recursive_directory_iterator iterator(root,
                                                           iterator_error);
    const std::filesystem::recursive_directory_iterator end;
    if (iterator_error) {
      model.diagnose({.path = relative_root.generic_string()},
                     "cannot scan project headers for LuaLS contracts: " +
                         iterator_error.message());
      continue;
    }
    while (iterator != end) {
      std::error_code type_error;
      const bool is_header = iterator->is_regular_file(type_error) &&
                             iterator->path().extension() == ".h";
      if (type_error) {
        model.diagnose(
            {.path = repository_path(repository_root, iterator->path())},
            "cannot inspect Lua type generator source candidate: " +
                type_error.message());
      } else if (is_header) {
        const std::string display_path =
            repository_path(repository_root, iterator->path());
        const std::optional<std::string> contents =
            read_binary(model, iterator->path(), display_path);
        if (contents.has_value())
          collect_contract_markers(model, display_path, *contents);
      }
      iterator.increment(iterator_error);
      if (iterator_error) {
        model.diagnose({.path = relative_root.generic_string()},
                       "cannot scan project headers for LuaLS contracts: " +
                           iterator_error.message());
        break;
      }
    }
  }
}

} // namespace

bool is_project_source(std::string_view path) {
  return path.starts_with("src/mux/") || path.starts_with("src/btech/");
}

bool is_mux_lua_source(std::string_view path) {
  return path.starts_with("src/mux/lua/");
}

bool is_mux_registration_source(std::string_view path) {
  return path.starts_with("src/mux/lua/packages/mux/") ||
         path == "src/mux/lua/lua_error.c" ||
         path == "src/mux/lua/command_access.c";
}

bool is_btech_binding_source(std::string_view path) {
  return path.starts_with("src/mux/lua/packages/btech/");
}

std::string contract_marker_identity(std::string_view path, size_t offset) {
  return std::string(path) + ":" + std::to_string(offset);
}

std::vector<std::string>
select_source_files(Model &model, const std::filesystem::path &repository_root,
                    const std::vector<std::string> &candidates,
                    const clang::tooling::CompilationDatabase &compilations) {
  scan_contract_headers(model, repository_root);

  // JSON compilation databases may synthesize an interpolated command for a
  // file they do not contain. A nonempty command inventory therefore becomes
  // the exact membership authority. FixedCompilationDatabase, used with
  // trailing `--` flags, reports no commands and intentionally applies to
  // every candidate.
  std::set<std::string> compilation_files;
  for (const clang::tooling::CompileCommand &command :
       compilations.getAllCompileCommands())
    compilation_files.insert(compile_command_path(command));

  std::vector<std::string> selected;
  for (const std::string &candidate : candidates) {
    const std::filesystem::path path(candidate);
    const std::string display_path = repository_path(repository_root, path);
    const std::optional<std::string> contents =
        read_binary(model, path, display_path);
    if (!contents.has_value())
      continue;
    collect_contract_markers(model, display_path, *contents);
    if (!is_mux_lua_source(display_path) &&
        !is_btech_binding_source(display_path) &&
        !has_source_trigger(*contents))
      continue;
    const bool missing_exact_command =
        !compilation_files.empty() && !compilation_files.contains(candidate);
    if (missing_exact_command ||
        compilations.getCompileCommands(candidate).empty()) {
      model.diagnose({.path = display_path},
                     "selected Lua type source has no compilation command");
      continue;
    }
    selected.push_back(candidate);
  }
  if (selected.empty())
    model.diagnose({.path = repository_root.generic_string()},
                   "no relevant Lua type source translation units selected");
  return selected;
}

} // namespace lua_types
