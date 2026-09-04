#pragma once

#include <map>
#include <set>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

namespace lua_types {

struct Location {
  std::string path;
  unsigned line = 1;
  unsigned column = 1;
};

struct Diagnostic {
  Location location;
  std::string message;

  auto tie() const {
    return std::tie(location.path, location.line, location.column, message);
  }
};

struct Contract {
  std::string module;
  std::string section;
  std::string key;
  std::string payload;
  std::string owner;
  std::string callable_target;
  Location location;
};

struct Ignore {
  std::string module;
  std::string leaf;
  std::string reason;
  std::string owner;
  Location location;
};

struct Registration {
  std::string module;
  std::string handler;
  std::string leaf;
  Location location;
};

struct BtechEntry {
  std::string name;
  std::string qualified_name;
  std::string handler;
  std::string inventory;
  Location location;
};

struct BtechInstallation {
  std::string inventory;
  std::string package;
  Location location;
};

struct DeferredRegistration {
  std::string installer;
  unsigned parameter = 0;
  std::string leaf;
  Location location;
};

struct InstallerCall {
  std::string installer;
  std::vector<std::string> argument_functions;
  Location location;
};

struct CatalogFact {
  std::string owner;
  std::set<std::string> values;
  Location location;
};

struct Model {
  std::string repository_root;
  std::vector<Contract> contracts;
  std::vector<Ignore> ignores;
  std::vector<Registration> registrations;
  std::vector<BtechEntry> btech_entries;
  std::map<std::string, Location> btech_inventories;
  std::vector<BtechInstallation> btech_installations;
  std::vector<DeferredRegistration> deferred_registrations;
  std::vector<InstallerCall> installer_calls;
  std::map<std::string, CatalogFact> catalogs;
  std::map<std::string, Location> expected_contract_markers;
  std::set<std::string> visited_contract_markers;
  std::set<std::string> seen_comments;
  std::map<std::string, Location> contract_comment_locations;
  std::set<std::tuple<std::string, unsigned, unsigned, std::string>>
      diagnostic_keys;
  std::vector<Diagnostic> diagnostics;

  void diagnose(const Location &location, std::string message) {
    const auto key =
        std::tuple(location.path, location.line, location.column, message);
    if (!diagnostic_keys.insert(key).second)
      return;
    diagnostics.push_back(
        {.location = location, .message = std::move(message)});
  }
};

} // namespace lua_types
