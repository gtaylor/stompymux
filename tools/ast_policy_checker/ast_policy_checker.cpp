#include <algorithm>
#include <cstdint>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <tuple>

#include "clang/AST/ASTContext.h"
#include "clang/AST/Decl.h"
#include "clang/ASTMatchers/ASTMatchFinder.h"
#include "clang/ASTMatchers/ASTMatchers.h"
#include "clang/Basic/SourceManager.h"
#include "clang/Frontend/FrontendActions.h"
#include "clang/Tooling/CommonOptionsParser.h"
#include "clang/Tooling/Tooling.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/raw_ostream.h"

namespace {

using clang::ASTContext;
using clang::BinaryOperator;
using clang::CallExpr;
using clang::FunctionDecl;
using clang::SourceLocation;
using clang::SourceManager;
using clang::ast_matchers::MatchFinder;
using namespace clang::ast_matchers;

llvm::cl::OptionCategory POLICY_CATEGORY("ast-policy-checker options");
llvm::cl::opt<std::string>
    CHECKS("checks", llvm::cl::desc("Policy to run"), llvm::cl::init("all"),
           llvm::cl::value_desc(
               "all|enum-underlying-type|status-accessors|checked-suffix-order|"
               "boolean-contracts"),
           llvm::cl::cat(POLICY_CATEGORY));

enum class Policy : std::uint8_t {
  all,
  enum_underlying_type,
  status_accessors,
  checked_suffix_order,
  boolean_contracts,
};

std::optional<Policy> parse_policy(std::string_view value) {
  if (value == "all") {
    return Policy::all;
  }
  if (value == "status-accessors") {
    return Policy::status_accessors;
  }
  if (value == "enum-underlying-type") {
    return Policy::enum_underlying_type;
  }
  if (value == "checked-suffix-order") {
    return Policy::checked_suffix_order;
  }
  if (value == "boolean-contracts") {
    return Policy::boolean_contracts;
  }
  return std::nullopt;
}

bool policy_enabled(Policy selected, Policy policy) {
  return selected == Policy::all || selected == policy;
}

bool is_production_source(const SourceManager &sources,
                          SourceLocation location) {
  const SourceLocation spelling = sources.getSpellingLoc(location);
  const std::string path = sources.getFilename(spelling).str();
  return path.starts_with("src/mux/") || path.starts_with("src/btech/") ||
         path.find("/src/mux/") != std::string::npos ||
         path.find("/src/btech/") != std::string::npos;
}

class PolicyReporter {
public:
  void report(const SourceManager &sources, SourceLocation location,
              std::string_view message) {
    const SourceLocation spelling = sources.getSpellingLoc(location);
    if (!spelling.isValid()) {
      return;
    }

    const std::string path = sources.getFilename(spelling).str();
    const unsigned line = sources.getSpellingLineNumber(spelling);
    const unsigned column = sources.getSpellingColumnNumber(spelling);
    const auto key = std::tuple(path, line, column, std::string(message));
    if (!diagnostics_.insert(key).second) {
      return;
    }

    llvm::errs() << path << ':' << line << ':' << column << ": " << message
                 << '\n';
  }

  [[nodiscard]] bool has_violations() const { return !diagnostics_.empty(); }

private:
  std::set<std::tuple<std::string, unsigned, unsigned, std::string>>
      diagnostics_;
};

class StatusAccessorCallback final : public MatchFinder::MatchCallback {
public:
  explicit StatusAccessorCallback(PolicyReporter &reporter)
      : reporter_(reporter) {}

  void run(const MatchFinder::MatchResult &result) override {
    const auto *operation = result.Nodes.getNodeAs<BinaryOperator>("operation");
    if (operation == nullptr || result.SourceManager == nullptr) {
      return;
    }
    reporter_.report(*result.SourceManager, operation->getOperatorLoc(),
                     "Raw bitwise operation on a persisted unit status word; "
                     "use its status-specific accessor.");
  }

private:
  PolicyReporter &reporter_;
};

class EnumUnderlyingTypeCallback final : public MatchFinder::MatchCallback {
public:
  explicit EnumUnderlyingTypeCallback(PolicyReporter &reporter)
      : reporter_(reporter) {}

  void run(const MatchFinder::MatchResult &result) override {
    const auto *declaration =
        result.Nodes.getNodeAs<clang::EnumDecl>("enum_declaration");
    if (declaration == nullptr || result.SourceManager == nullptr ||
        declaration->getIdentifier() == nullptr || declaration->isFixed() ||
        !is_production_source(*result.SourceManager,
                              declaration->getLocation())) {
      return;
    }
    reporter_.report(*result.SourceManager, declaration->getLocation(),
                     "Named enum without an explicit underlying type found; "
                     "use : int.");
  }

private:
  PolicyReporter &reporter_;
};

class CheckedSuffixCallback final : public MatchFinder::MatchCallback {
public:
  explicit CheckedSuffixCallback(PolicyReporter &reporter)
      : reporter_(reporter) {}

  void run(const MatchFinder::MatchResult &result) override {
    const auto *call = result.Nodes.getNodeAs<CallExpr>("suffix_call");
    const auto *write = result.Nodes.getNodeAs<BinaryOperator>("write");
    if (call == nullptr || write == nullptr ||
        result.SourceManager == nullptr) {
      return;
    }

    const SourceManager &sources = *result.SourceManager;
    if (!is_production_source(sources, call->getExprLoc())) {
      return;
    }

    const SourceLocation call_location =
        sources.getSpellingLoc(call->getExprLoc());
    const SourceLocation write_location =
        sources.getSpellingLoc(write->getOperatorLoc());
    if (!sources.isWrittenInSameFile(call_location, write_location) ||
        !sources.isBeforeInTranslationUnit(write_location, call_location)) {
      return;
    }

    reporter_.report(sources, write->getOperatorLoc(),
                     "NUL write precedes a checked suffix of the same pointer; "
                     "capture the suffix first.");
  }

private:
  PolicyReporter &reporter_;
};

class BooleanContractCallback final : public MatchFinder::MatchCallback {
public:
  explicit BooleanContractCallback(PolicyReporter &reporter)
      : reporter_(reporter) {}

  void run(const MatchFinder::MatchResult &result) override {
    const auto *function = result.Nodes.getNodeAs<FunctionDecl>("function");
    if (function == nullptr || result.SourceManager == nullptr ||
        !is_production_source(*result.SourceManager, function->getLocation())) {
      return;
    }
    reporter_.report(*result.SourceManager, function->getLocation(),
                     "Integer-returning predicate found; use a bool contract "
                     "or document an exemption.");
  }

private:
  PolicyReporter &reporter_;
};

auto status_accessor_matcher() {
  const auto status_type = qualType(hasDeclaration(namedDecl(
      hasAnyName("MechStatus", "MechStatus2", "MechSpecialsStatus",
                 "MechCritStatus", "MechTankCritStatus", "MechCritStatus2"))));
  return binaryOperator(isExpansionInMainFile(),
                        hasAnyOperatorName("&", "|", "^"),
                        hasEitherOperand(ignoringParenImpCasts(anyOf(
                            memberExpr(hasType(status_type)),
                            declRefExpr(to(varDecl(hasType(status_type))))))))
      .bind("operation");
}

auto enum_underlying_type_matcher() {
  return enumDecl(isDefinition(), unless(isExpansionInSystemHeader()))
      .bind("enum_declaration");
}

auto checked_suffix_matcher() {
  const auto direct_write = unaryOperator(
      hasOperatorName("*"), hasUnaryOperand(ignoringParenImpCasts(
                                declRefExpr(to(equalsBoundNode("pointer"))))));
  const auto indexed_write = arraySubscriptExpr(hasBase(
      ignoringParenImpCasts(declRefExpr(to(equalsBoundNode("pointer"))))));
  const auto pointer_write =
      binaryOperator(
          isAssignmentOperator(),
          hasRHS(ignoringParenImpCasts(
              anyOf(characterLiteral(equals(0)), integerLiteral(equals(0))))),
          hasLHS(expr(anyOf(direct_write, indexed_write))))
          .bind("write");

  return callExpr(
             isExpansionInMainFile(),
             callee(functionDecl(hasAnyName("checked_string_suffix",
                                            "checked_mutable_string_suffix"))),
             hasArgument(
                 0, declRefExpr(
                        to(varDecl(hasType(pointerType())).bind("pointer")))),
             hasAncestor(functionDecl(
                 isExpansionInMainFile(),
                 hasBody(compoundStmt(hasDescendant(pointer_write))))))
      .bind("suffix_call");
}

auto boolean_contract_matcher() {
  return functionDecl(
             isExpansionInMainFile(), hasBody(stmt()), returns(asString("int")),
             unless(hasAnyName("repair_part_type_difficulty", "safe_copy_chr",
                               "lua_runtime_exit_enter_lock_passes")),
             unless(hasAnyParameter(hasType(pointerType(
                 pointee(typedefType(hasDeclaration(typedefDecl(hasAnyName(
                     "RepairOperationCall", "ConfigurationCall"))))))))),
             unless(hasAnyParameter(hasType(pointerType(
                 pointee(hasDeclaration(namedDecl(hasName("lua_State")))))))),
             hasDescendant(returnStmt()),
             unless(hasDescendant(
                 returnStmt(hasReturnValue(ignoringImpCasts(unless(
                     anyOf(integerLiteral(anyOf(equals(0), equals(1))),
                           binaryOperator(hasAnyOperatorName(
                               "&&", "||", "==", "!=", "<", ">", "<=", ">=")),
                           unaryOperator(hasOperatorName("!")),
                           expr(hasType(booleanType()))))))))))
      .bind("function");
}

} // namespace

int main(int argc, const char **argv) {
  auto options = clang::tooling::CommonOptionsParser::create(
      argc, argv, POLICY_CATEGORY, llvm::cl::OneOrMore);
  if (!options) {
    llvm::errs() << llvm::toString(options.takeError()) << '\n';
    return 2;
  }

  const std::optional<Policy> selected = parse_policy(CHECKS);
  if (!selected.has_value()) {
    llvm::errs() << "unknown policy: " << CHECKS << '\n';
    return 2;
  }

  PolicyReporter reporter;
  EnumUnderlyingTypeCallback enum_callback(reporter);
  StatusAccessorCallback status_callback(reporter);
  CheckedSuffixCallback suffix_callback(reporter);
  BooleanContractCallback boolean_callback(reporter);
  MatchFinder finder;
  if (policy_enabled(*selected, Policy::enum_underlying_type)) {
    finder.addMatcher(enum_underlying_type_matcher(), &enum_callback);
  }
  if (policy_enabled(*selected, Policy::status_accessors)) {
    finder.addMatcher(status_accessor_matcher(), &status_callback);
  }
  if (policy_enabled(*selected, Policy::checked_suffix_order)) {
    finder.addMatcher(checked_suffix_matcher(), &suffix_callback);
  }
  if (policy_enabled(*selected, Policy::boolean_contracts)) {
    finder.addMatcher(boolean_contract_matcher(), &boolean_callback);
  }

  clang::tooling::ClangTool tool(options->getCompilations(),
                                 options->getSourcePathList());
  const int tool_status =
      tool.run(clang::tooling::newFrontendActionFactory(&finder).get());
  if (tool_status != 0) {
    return tool_status;
  }
  return reporter.has_violations() ? 1 : 0;
}
