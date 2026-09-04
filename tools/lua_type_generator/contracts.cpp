#include "contracts.h"

#include <algorithm>
#include <cctype>
#include <functional>
#include <map>
#include <regex>
#include <set>
#include <sstream>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace lua_types {
namespace {

const std::map<std::string, std::string> CATALOG_CLASSES = {
    {"mux.world.locks", "LockNamespace"},
    {"mux.world.access", "AccessNamespace"},
    {"mux.world.flags", "FlagNamespace"},
    {"mux.world.powers", "PowerNamespace"},
    {"mux.comsys.flags", "ChannelFlagNamespace"},
    {"mux.world.types", "ObjectTypeNamespace"},
};

struct ErrorCatalogSchema {
  std::string prefix;
  std::string root_class;
};

const std::map<std::string, ErrorCatalogSchema> ERROR_CATALOG_SCHEMAS = {
    {"mux.error.codes", {.prefix = "mux.", .root_class = "MuxErrorCodes"}},
    {"btech.error.codes",
     {.prefix = "btech.", .root_class = "BtechErrorCodes"}},
    {"mux.testing.codes",
     {.prefix = "testing.", .root_class = "TestingErrorCodes"}},
};

bool starts_with(std::string_view value, std::string_view prefix) {
  return value.size() >= prefix.size() &&
         value.substr(0, prefix.size()) == prefix;
}

std::vector<std::string> comment_lines(std::string_view raw) {
  std::vector<std::string> result;
  size_t cursor = 0;
  while (cursor <= raw.size()) {
    const size_t newline = raw.find('\n', cursor);
    const size_t end = newline == std::string_view::npos ? raw.size() : newline;
    std::string line(raw.substr(cursor, end - cursor));
    if (!line.empty() && line.back() == '\r')
      line.pop_back();
    const size_t first = line.find_first_not_of(" \t");
    if (first != std::string::npos)
      line.erase(0, first);
    else
      line.clear();

    if (result.empty() && starts_with(line, "/**")) {
      line.erase(0, 3);
      if (!line.empty() && line.front() == ' ')
        line.erase(0, 1);
    } else if (starts_with(line, "*")) {
      line.erase(0, 1);
      if (!line.empty() && line.front() == ' ')
        line.erase(0, 1);
    }
    if (line == "*/")
      line.clear();
    else if (line.size() >= 2 && line.ends_with("*/")) {
      line.erase(line.size() - 2);
      if (!line.empty() && line.back() == ' ')
        line.pop_back();
    }
    result.push_back(std::move(line));
    if (newline == std::string_view::npos)
      break;
    cursor = newline + 1;
  }
  return result;
}

bool valid_payload_line(const std::string &line) {
  static const std::regex local_table(
      R"(^local [A-Za-z_][A-Za-z0-9_]* = \{\}$)");
  static const std::regex global_table(R"(^[A-Za-z_][A-Za-z0-9_]* = \{\}$)");
  static const std::regex function(
      R"(^function [A-Za-z_][A-Za-z0-9_.:]*\([^\r\n]*\) end$)");
  static const std::regex assignment(
      R"(^[A-Za-z_][A-Za-z0-9_.]* = [A-Za-z_][A-Za-z0-9_]*$)");

  return line.empty() || starts_with(line, "---") ||
         std::regex_match(line, local_table) ||
         std::regex_match(line, global_table) ||
         std::regex_match(line, function) || std::regex_match(line, assignment);
}

std::string annotation_name(const std::string &line) {
  constexpr std::string_view prefix = "---@";
  if (!starts_with(line, prefix))
    return {};
  const size_t end = line.find_first_of(" \t", prefix.size());
  return line.substr(prefix.size(), end - prefix.size());
}

bool allowed_annotation(const std::string &section,
                        const std::string &annotation) {
  static const std::map<std::string, std::set<std::string>> allowed = {
      {"alias", {"alias", "see"}},
      {"type", {"class", "field", "operator", "see"}},
      {"catalog", {"class", "field", "operator", "see"}},
      {"namespace", {"class", "field", "see"}},
      {"callable", {"generic", "overload", "param", "return", "see"}},
      {"binding", {}},
  };
  return allowed.at(section).contains(annotation);
}

bool valid_payload_line_for_section(const std::string &line,
                                    const std::string &section) {
  if (line.empty() || (starts_with(line, "---") && !starts_with(line, "---@")))
    return true;
  const std::string annotation = annotation_name(line);
  if (!annotation.empty())
    return allowed_annotation(section, annotation);
  if (starts_with(line, "local "))
    return section == "type" || section == "namespace";
  if (starts_with(line, "function "))
    return section == "callable";
  return section == "namespace" || section == "binding";
}

std::string symbol_leaf(std::string_view value) {
  const size_t separator = value.find_last_of(".:");
  return std::string(separator == std::string_view::npos
                         ? value
                         : value.substr(separator + 1));
}

std::string function_target(const std::string &line) {
  constexpr std::string_view prefix = "function ";
  if (!starts_with(line, prefix))
    return {};
  const size_t open = line.find('(', prefix.size());
  if (open == std::string::npos)
    return {};
  return line.substr(prefix.size(), open - prefix.size());
}

Location line_location(const Location &base, size_t offset) {
  Location result = base;
  result.line += static_cast<unsigned>(offset);
  result.column = 1;
  return result;
}

struct CatalogProjection {
  std::set<std::string> values;
  std::set<std::string> root_fields;
  std::set<std::string> tree_values;
  bool has_duplicate = false;
  bool has_invalid_tree = false;
};

struct ErrorClass {
  std::string code;
  std::map<std::string, std::string> fields;
};

std::string declared_class_name(const std::string &line) {
  constexpr std::string_view prefix = "---@class ";
  if (!starts_with(line, prefix))
    return {};
  std::string declaration = line.substr(prefix.size());
  const size_t colon = declaration.find(':');
  if (colon != std::string::npos)
    declaration.erase(colon);
  const size_t space = declaration.find_last_of(' ');
  return declaration.substr(space == std::string::npos ? 0 : space + 1);
}

CatalogProjection catalog_contract_values(const Contract &contract,
                                          const std::string &catalog) {
  CatalogProjection projection;
  std::istringstream stream(contract.payload);
  std::string line;
  const auto error_schema = ERROR_CATALOG_SCHEMAS.find(catalog);
  if (error_schema != ERROR_CATALOG_SCHEMAS.end()) {
    static const std::regex literal_code(
        R"REGEX(^---@field code "([^"]+)"(?: .*)?$)REGEX");
    static const std::regex field(
        R"(^---@field ([A-Za-z_][A-Za-z0-9_]*)\b.*$)");
    static const std::regex typed_field(
        R"(^---@field ([A-Za-z_][A-Za-z0-9_]*) ([A-Za-z_][A-Za-z0-9_]*)(?: .*)?$)");
    std::smatch match;
    std::map<std::string, ErrorClass> classes;
    std::string current_class;
    bool in_root_class = false;
    while (std::getline(stream, line)) {
      if (starts_with(line, "---@class ")) {
        current_class = declared_class_name(line);
        if (!classes.try_emplace(current_class).second)
          projection.has_duplicate = true;
        in_root_class = current_class == error_schema->second.root_class;
        continue;
      }
      if (std::regex_match(line, match, literal_code)) {
        const std::string code = match[1].str();
        if (!projection.values.insert(code).second)
          projection.has_duplicate = true;
        if (current_class.empty() || !classes[current_class].code.empty())
          projection.has_invalid_tree = true;
        else
          classes[current_class].code = code;
      } else if (std::regex_match(line, match, typed_field)) {
        if (current_class.empty() ||
            !classes[current_class]
                 .fields.emplace(match[1].str(), match[2].str())
                 .second)
          projection.has_duplicate = true;
      }
      if (in_root_class && std::regex_match(line, match, field) &&
          !projection.root_fields.insert(match[1].str()).second)
        projection.has_duplicate = true;
    }

    std::set<std::string> visiting;
    std::function<void(const std::string &, const std::string &)> visit_class;
    visit_class = [&](const std::string &class_name, const std::string &path) {
      const auto found = classes.find(class_name);
      if (found == classes.end() || !visiting.insert(class_name).second) {
        projection.has_invalid_tree = true;
        return;
      }
      const ErrorClass &definition = found->second;
      if (!definition.code.empty()) {
        if (!definition.fields.empty() || definition.code != path)
          projection.has_invalid_tree = true;
        projection.tree_values.insert(path);
      } else {
        for (const auto &[field_name, field_type] : definition.fields)
          visit_class(field_type, path + "." + field_name);
      }
      visiting.erase(class_name);
    };
    std::string root = error_schema->second.prefix;
    root.pop_back();
    visit_class(error_schema->second.root_class, root);
    return projection;
  }

  const auto class_it = CATALOG_CLASSES.find(catalog);
  if (class_it == CATALOG_CLASSES.end())
    return projection;
  const std::string &wanted_class = class_it->second;
  static const std::regex field(R"(^---@field ([A-Za-z_][A-Za-z0-9_]*)\b.*$)");
  bool in_class = false;
  std::smatch match;
  while (std::getline(stream, line)) {
    if (starts_with(line, "---@class ")) {
      const std::string declaration = line.substr(10);
      const size_t colon = declaration.find(':');
      const std::string before_colon = declaration.substr(0, colon);
      in_class = before_colon == wanted_class ||
                 before_colon.ends_with(" " + wanted_class);
      continue;
    }
    if (in_class && std::regex_match(line, match, field) &&
        !projection.values.insert(match[1].str()).second)
      projection.has_duplicate = true;
  }
  return projection;
}

std::string join_values(const std::set<std::string> &values) {
  std::string result;
  for (const std::string &value : values) {
    if (!result.empty())
      result += ", ";
    result += value;
  }
  return result;
}

std::set<std::string> error_root_fields(const std::set<std::string> &values,
                                        const ErrorCatalogSchema &schema) {
  std::set<std::string> result;
  for (const std::string &value : values) {
    if (!starts_with(value, schema.prefix))
      continue;
    const std::string_view suffix(value.data() + schema.prefix.size(),
                                  value.size() - schema.prefix.size());
    const size_t dot = suffix.find('.');
    result.emplace(suffix.substr(0, dot));
  }
  return result;
}

} // namespace

void parse_contract_comment(Model &model, std::string_view raw,
                            const std::string &owner,
                            const Location &location) {
  if (raw.find("@par LuaLS") == std::string_view::npos)
    return;
  if (raw.find('\r') != std::string_view::npos)
    model.diagnose(location, "LuaLS contract contains a carriage return");
  const std::vector<std::string> lines = comment_lines(raw);
  const std::regex definition(
      R"(^@par LuaLS definition (mux|btech) (alias|type|catalog|namespace|callable|binding) ([A-Za-z_][A-Za-z0-9_.:-]*)$)");
  const std::regex ignore(
      R"(^@par LuaLS ignore (mux) ([A-Za-z_][A-Za-z0-9_]*) -- (.+)$)");

  for (size_t index = 0; index < lines.size(); index++) {
    if (!starts_with(lines[index], "@par LuaLS"))
      continue;
    const Location current = line_location(location, index);
    std::smatch match;
    if (lines[index].find('\t') != std::string::npos ||
        (!lines[index].empty() &&
         std::isspace(static_cast<unsigned char>(lines[index].back())))) {
      model.diagnose(current,
                     "LuaLS metadata contains a tab or trailing whitespace");
    }
    if (std::regex_match(lines[index], match, ignore)) {
      const std::string reason = match[3].str();
      if (std::all_of(reason.begin(), reason.end(), [](unsigned char value) {
            return std::isspace(value);
          })) {
        model.diagnose(
            current,
            "LuaLS ignore reason must contain a non-whitespace character");
      }
      model.ignores.push_back({.module = match[1].str(),
                               .leaf = match[2].str(),
                               .reason = reason,
                               .owner = owner,
                               .location = current});
      continue;
    }
    if (!std::regex_match(lines[index], match, definition)) {
      model.diagnose(current, "malformed LuaLS contract header");
      continue;
    }
    const std::string module = match[1].str();
    const std::string section = match[2].str();
    const std::string key = match[3].str();
    if (index + 1 >= lines.size() || lines[index + 1] != "@code{.lua}") {
      model.diagnose(current,
                     "LuaLS definition must be followed by @code{.lua}");
      continue;
    }
    const size_t payload_start = index + 2;
    size_t payload_end = payload_start;
    while (payload_end < lines.size() && lines[payload_end] != "@endcode")
      payload_end++;
    if (payload_end == lines.size()) {
      model.diagnose(current, "LuaLS definition is missing @endcode");
      break;
    }
    if (payload_start == payload_end || lines[payload_start].empty() ||
        lines[payload_end - 1].empty()) {
      model.diagnose(current,
                     "LuaLS payload must be nonempty with no edge blank line");
      index = payload_end;
      continue;
    }

    std::string payload;
    std::string callable_target;
    unsigned function_count = 0;
    unsigned alias_count = 0;
    unsigned class_count = 0;
    unsigned table_count = 0;
    unsigned assignment_count = 0;
    for (size_t line_index = payload_start; line_index < payload_end;
         line_index++) {
      const std::string &line = lines[line_index];
      if (line.find('\r') != std::string::npos ||
          line.find('\t') != std::string::npos ||
          (!line.empty() &&
           std::isspace(static_cast<unsigned char>(line.back())))) {
        model.diagnose(
            line_location(location, line_index),
            "LuaLS payload contains CR, tab, or trailing whitespace");
      }
      if (!valid_payload_line(line)) {
        model.diagnose(line_location(location, line_index),
                       "unsupported LuaLS payload line");
      } else if (!valid_payload_line_for_section(line, section)) {
        model.diagnose(line_location(location, line_index),
                       "LuaLS payload line is not allowed in " + section +
                           " blocks");
      }
      if (starts_with(line, "---@meta") || starts_with(line, "return ")) {
        model.diagnose(line_location(location, line_index),
                       "LuaLS payload may not own the generated envelope");
      }
      const std::string target = function_target(line);
      if (!target.empty()) {
        function_count++;
        callable_target = target;
      }
      const std::string annotation = annotation_name(line);
      if (annotation == "alias")
        alias_count++;
      if (annotation == "class")
        class_count++;
      if (starts_with(line, "local ") ||
          (section == "namespace" && !line.empty() && line[0] != '-' &&
           annotation.empty() && line.find(" = ") != std::string::npos))
        table_count++;
      if (section == "binding" && !line.empty() && line[0] != '-' &&
          annotation.empty() && line.find(" = ") != std::string::npos)
        assignment_count++;
      if (!payload.empty())
        payload.push_back('\n');
      payload += line;
    }

    if (section == "callable") {
      if (function_count != 1) {
        model.diagnose(current,
                       "callable LuaLS payload must contain one function line");
      } else if (symbol_leaf(key) != symbol_leaf(callable_target)) {
        model.diagnose(
            current, "callable key and emitted function have different leaves");
      }
      if (module == "btech") {
        const std::string_view prefix = "btech.";
        const size_t package_end = key.find('.', prefix.size());
        if (!starts_with(key, prefix) || package_end == std::string::npos ||
            key.find('.', package_end + 1) != std::string::npos) {
          model.diagnose(current,
                         "BTech callable key must be btech.<package>.<leaf>");
        } else {
          const std::string expected =
              "btech_" +
              key.substr(prefix.size(), package_end - prefix.size()) + "." +
              key.substr(package_end + 1);
          if (callable_target != expected)
            model.diagnose(current, "BTech callable emits " + callable_target +
                                        "; expected " + expected);
        }
      }
    } else if (function_count != 0) {
      model.diagnose(current,
                     "function declarations belong in callable blocks");
    }
    if (section == "alias" && alias_count == 0)
      model.diagnose(current, "alias LuaLS payload must declare an alias");
    if ((section == "type" || section == "catalog") && class_count == 0)
      model.diagnose(current, section + " LuaLS payload must declare a class");
    if (section == "namespace" && table_count == 0)
      model.diagnose(current,
                     "namespace LuaLS payload must declare a namespace table");
    if (section == "binding" && assignment_count == 0)
      model.diagnose(current,
                     "binding LuaLS payload must contain an assignment");

    model.contracts.push_back({.module = module,
                               .section = section,
                               .key = key,
                               .payload = std::move(payload),
                               .owner = owner,
                               .callable_target = std::move(callable_target),
                               .location = current});
    index = payload_end;
  }
}

void validate_model(Model &model) {
  for (const auto &[identity, location] : model.expected_contract_markers) {
    if (!model.visited_contract_markers.contains(identity))
      model.diagnose(
          location,
          "LuaLS contract marker was not visited by a selected translation "
          "unit");
  }

  for (const auto &[identity, location] : model.contract_comment_locations) {
    if (!model.seen_comments.contains(identity))
      model.diagnose(location,
                     "LuaLS contract comment is not attached to a declaration");
  }

  for (const DeferredRegistration &deferred : model.deferred_registrations) {
    bool resolved = false;
    for (const InstallerCall &call : model.installer_calls) {
      if (call.installer != deferred.installer ||
          deferred.parameter >= call.argument_functions.size() ||
          call.argument_functions[deferred.parameter].empty())
        continue;
      model.registrations.push_back(
          {.module = "mux",
           .handler = call.argument_functions[deferred.parameter],
           .leaf = deferred.leaf,
           .location = call.location});
      resolved = true;
    }
    if (!resolved)
      model.diagnose(deferred.location,
                     "could not resolve Lua installer function parameter");
  }

  std::map<std::pair<std::string, std::string>, Location> contract_keys;
  for (const Contract &contract : model.contracts) {
    const auto key = std::pair(contract.module, contract.key);
    if (!contract_keys.emplace(key, contract.location).second)
      model.diagnose(contract.location, "duplicate LuaLS definition key");
  }

  std::map<std::pair<std::string, std::string>, const Contract *> mux_callable;
  for (const Contract &contract : model.contracts) {
    if (contract.module != "mux" || contract.section != "callable")
      continue;
    const auto key = std::pair(contract.owner, symbol_leaf(contract.key));
    if (!mux_callable.emplace(key, &contract).second)
      model.diagnose(contract.location,
                     "duplicate MUX handler/leaf callable disposition");
  }
  std::map<std::pair<std::string, std::string>, const Ignore *> mux_ignore;
  for (const Ignore &ignore : model.ignores) {
    const auto key = std::pair(ignore.owner, ignore.leaf);
    if (!mux_ignore.emplace(key, &ignore).second)
      model.diagnose(ignore.location, "duplicate MUX handler/leaf ignore");
    if (mux_callable.contains(key))
      model.diagnose(ignore.location,
                     "MUX handler/leaf has both callable and ignore records");
  }
  std::set<std::pair<std::string, std::string>> mux_registrations;
  for (const Registration &registration : model.registrations) {
    if (registration.module != "mux")
      continue;
    const auto key = std::pair(registration.handler, registration.leaf);
    mux_registrations.insert(key);
    if (!mux_callable.contains(key) && !mux_ignore.contains(key))
      model.diagnose(registration.location,
                     "registered MUX handler/leaf lacks a LuaLS callable or "
                     "documented ignore: " +
                         registration.leaf);
  }
  for (const auto &[key, contract] : mux_callable) {
    if (!mux_registrations.contains(key))
      model.diagnose(contract->location,
                     "MUX callable is not backed by a recognized registration");
  }
  for (const auto &[key, ignore] : mux_ignore) {
    if (!mux_registrations.contains(key))
      model.diagnose(ignore->location,
                     "MUX ignore is not backed by a recognized registration");
  }

  std::map<std::string, const BtechInstallation *> btech_installations;
  for (const BtechInstallation &installation : model.btech_installations) {
    if (!model.btech_inventories.contains(installation.inventory)) {
      model.diagnose(installation.location,
                     "BTech installer references an unknown entry inventory");
      continue;
    }
    if (!btech_installations.emplace(installation.inventory, &installation)
             .second)
      model.diagnose(
          installation.location,
          "BtechLuaNativeEntry inventory is installed more than once");
  }
  for (const auto &[inventory, location] : model.btech_inventories) {
    if (!btech_installations.contains(inventory))
      model.diagnose(
          location,
          "BtechLuaNativeEntry inventory has no recognized installer");
  }

  std::map<std::string, const BtechEntry *> btech_names;
  std::set<std::string> btech_expected;
  for (const BtechEntry &entry : model.btech_entries) {
    if (symbol_leaf(entry.qualified_name) != entry.name)
      model.diagnose(
          entry.location,
          "BtechLuaNativeEntry short name differs from qualified leaf");
    const auto installation = btech_installations.find(entry.inventory);
    if (installation == btech_installations.end())
      continue;
    const std::string runtime_name =
        installation->second->package + "." + entry.name;
    if (entry.qualified_name != runtime_name)
      model.diagnose(entry.location, "BtechLuaNativeEntry qualified name "
                                     "differs from installer package/name");
    const std::string public_name = "btech." + runtime_name;
    if (!btech_names.emplace(public_name, &entry).second)
      model.diagnose(entry.location, "duplicate registered BTech name");
    btech_expected.emplace(public_name);
  }
  std::set<std::string> btech_actual;
  for (const Contract &contract : model.contracts) {
    if (contract.module != "btech" || contract.section != "callable")
      continue;
    btech_actual.insert(contract.key);
    if (!btech_expected.contains(contract.key))
      model.diagnose(
          contract.location,
          "BTech callable does not exactly match a BtechLuaNativeEntry");
  }
  for (const BtechEntry &entry : model.btech_entries) {
    const std::string key = "btech." + entry.qualified_name;
    if (!btech_actual.contains(key))
      model.diagnose(
          entry.location,
          "BtechLuaNativeEntry lacks its exact LuaLS callable contract");
  }

  std::map<std::string, const Contract *> catalog_contracts;
  for (const Contract &contract : model.contracts) {
    if (contract.section != "catalog")
      continue;
    if (!catalog_contracts.emplace(contract.key, &contract).second)
      model.diagnose(contract.location, "duplicate LuaLS catalog contract");
    if (!model.catalogs.contains(contract.key))
      model.diagnose(contract.location,
                     "LuaLS catalog has no recognized native owner");
  }
  for (const auto &[name, fact] : model.catalogs) {
    const auto contract_it = catalog_contracts.find(name);
    if (contract_it == catalog_contracts.end()) {
      model.diagnose(fact.location,
                     "native Lua catalog lacks a LuaLS catalog block: " + name);
      continue;
    }
    const Contract &contract = *contract_it->second;
    if (contract.owner != fact.owner)
      model.diagnose(contract.location,
                     "LuaLS catalog is not attached to its native owner");
    const CatalogProjection actual = catalog_contract_values(contract, name);
    if (actual.has_duplicate)
      model.diagnose(contract.location,
                     "LuaLS catalog contains a duplicate projected value");
    if (actual.values != fact.values) {
      std::set<std::string> missing;
      std::set<std::string> extra;
      std::set_difference(fact.values.begin(), fact.values.end(),
                          actual.values.begin(), actual.values.end(),
                          std::inserter(missing, missing.end()));
      std::set_difference(actual.values.begin(), actual.values.end(),
                          fact.values.begin(), fact.values.end(),
                          std::inserter(extra, extra.end()));
      std::string message = "LuaLS catalog differs from native values";
      if (!missing.empty())
        message += "; missing: " + join_values(missing);
      if (!extra.empty())
        message += "; extra: " + join_values(extra);
      model.diagnose(contract.location, std::move(message));
    }
    const auto schema = ERROR_CATALOG_SCHEMAS.find(name);
    if (schema != ERROR_CATALOG_SCHEMAS.end()) {
      if (actual.has_invalid_tree)
        model.diagnose(
            contract.location,
            "LuaLS error catalog tree has invalid class references or code "
            "paths");
      if (actual.tree_values != fact.values) {
        std::set<std::string> missing;
        std::set<std::string> extra;
        std::set_difference(
            fact.values.begin(), fact.values.end(), actual.tree_values.begin(),
            actual.tree_values.end(), std::inserter(missing, missing.end()));
        std::set_difference(actual.tree_values.begin(),
                            actual.tree_values.end(), fact.values.begin(),
                            fact.values.end(),
                            std::inserter(extra, extra.end()));
        std::string message =
            "LuaLS error catalog tree differs from native values";
        if (!missing.empty())
          message += "; missing: " + join_values(missing);
        if (!extra.empty())
          message += "; extra: " + join_values(extra);
        model.diagnose(contract.location, std::move(message));
      }
      const std::set<std::string> expected_fields =
          error_root_fields(fact.values, schema->second);
      if (actual.root_fields != expected_fields) {
        std::set<std::string> missing;
        std::set<std::string> extra;
        std::set_difference(expected_fields.begin(), expected_fields.end(),
                            actual.root_fields.begin(),
                            actual.root_fields.end(),
                            std::inserter(missing, missing.end()));
        std::set_difference(actual.root_fields.begin(),
                            actual.root_fields.end(), expected_fields.begin(),
                            expected_fields.end(),
                            std::inserter(extra, extra.end()));
        std::string message =
            "LuaLS error catalog root fields differ from native values";
        if (!missing.empty())
          message += "; missing: " + join_values(missing);
        if (!extra.empty())
          message += "; extra: " + join_values(extra);
        model.diagnose(contract.location, std::move(message));
      }
    }
  }
}

} // namespace lua_types
