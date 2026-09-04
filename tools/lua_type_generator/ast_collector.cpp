#include "ast_collector.h"

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "clang/AST/ASTConsumer.h"
#include "clang/AST/ASTContext.h"
#include "clang/AST/Decl.h"
#include "clang/AST/Expr.h"
#include "clang/AST/RecursiveASTVisitor.h"
#include "clang/AST/Stmt.h"
#include "clang/Basic/SourceManager.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Frontend/FrontendAction.h"
#include "clang/Index/USRGeneration.h"
#include "clang/Lex/Lexer.h"
#include "clang/Lex/Preprocessor.h"
#include "llvm/ADT/SmallString.h"

#include "contracts.h"
#include "source_selection.h"

namespace lua_types {
namespace {

using clang::ASTContext;
using clang::CallExpr;
using clang::CompoundStmt;
using clang::Decl;
using clang::DeclRefExpr;
using clang::Expr;
using clang::FunctionDecl;
using clang::ImplicitCastExpr;
using clang::InitListExpr;
using clang::MemberExpr;
using clang::ParmVarDecl;
using clang::RawComment;
using clang::SourceLocation;
using clang::SourceManager;
using clang::Stmt;
using clang::StringLiteral;
using clang::UnaryOperator;
using clang::VarDecl;

std::string normalized_path(const Model &model, const SourceManager &sources,
                            SourceLocation location) {
  location = sources.getSpellingLoc(location);
  std::filesystem::path path(sources.getFilename(location).str());
  if (path.empty())
    return "<unknown>";
  if (path.is_relative())
    path = std::filesystem::path(model.repository_root) / path;
  path = path.lexically_normal();
  const std::filesystem::path root =
      std::filesystem::path(model.repository_root).lexically_normal();
  std::error_code error;
  const std::filesystem::path relative =
      std::filesystem::relative(path, root, error);
  if (!error && !relative.empty() && !relative.string().starts_with(".."))
    return relative.generic_string();
  return path.generic_string();
}

Location source_location(const Model &model, const SourceManager &sources,
                         SourceLocation location) {
  const SourceLocation spelling = sources.getSpellingLoc(location);
  if (!spelling.isValid())
    return {.path = "<unknown>"};
  return {.path = normalized_path(model, sources, spelling),
          .line = sources.getSpellingLineNumber(spelling),
          .column = sources.getSpellingColumnNumber(spelling)};
}

std::string declaration_usr(const Decl *declaration,
                            const SourceManager &sources, const Model &model) {
  llvm::SmallString<128> buffer;
  if (!clang::index::generateUSRForDecl(declaration, buffer))
    return std::string(buffer);
  const Location location =
      source_location(model, sources, declaration->getLocation());
  std::string name;
  if (const auto *named = llvm::dyn_cast<clang::NamedDecl>(declaration))
    name = named->getNameAsString();
  return location.path + ":" + std::to_string(location.line) + ":" + name;
}

const Expr *strip_expression(const Expr *expression) {
  if (expression == nullptr)
    return nullptr;
  expression = expression->IgnoreParenImpCasts();
  while (const auto *cast = llvm::dyn_cast<ImplicitCastExpr>(expression))
    expression = cast->getSubExpr()->IgnoreParenImpCasts();
  if (const auto *unary = llvm::dyn_cast<UnaryOperator>(expression)) {
    if (unary->getOpcode() == clang::UO_AddrOf)
      return strip_expression(unary->getSubExpr());
  }
  return expression;
}

const FunctionDecl *function_expression(const Expr *expression) {
  expression = strip_expression(expression);
  const auto *reference = llvm::dyn_cast_or_null<DeclRefExpr>(expression);
  return reference == nullptr
             ? nullptr
             : llvm::dyn_cast<FunctionDecl>(reference->getDecl());
}

const ParmVarDecl *parameter_expression(const Expr *expression) {
  expression = strip_expression(expression);
  const auto *reference = llvm::dyn_cast_or_null<DeclRefExpr>(expression);
  return reference == nullptr
             ? nullptr
             : llvm::dyn_cast<ParmVarDecl>(reference->getDecl());
}

const VarDecl *variable_expression(const Expr *expression) {
  expression = strip_expression(expression);
  const auto *reference = llvm::dyn_cast_or_null<DeclRefExpr>(expression);
  return reference == nullptr ? nullptr
                              : llvm::dyn_cast<VarDecl>(reference->getDecl());
}

const StringLiteral *string_expression(const Expr *expression) {
  return llvm::dyn_cast_or_null<StringLiteral>(strip_expression(expression));
}

const MemberExpr *member_expression(const Expr *expression) {
  return llvm::dyn_cast_or_null<MemberExpr>(strip_expression(expression));
}

bool is_channel_method_member(const Expr *expression,
                              llvm::StringRef member_name) {
  const MemberExpr *member = member_expression(expression);
  if (member == nullptr ||
      member->getMemberNameInfo().getAsString() != member_name)
    return false;
  return member->getBase()->getType().getAsString().find(
             "LuaMuxChannelMethod") != std::string::npos;
}

std::string called_name(const CallExpr *call) {
  if (call == nullptr)
    return {};
  const FunctionDecl *callee = call->getDirectCallee();
  return callee == nullptr ? std::string() : callee->getNameAsString();
}

bool is_named_member(const Expr *expression, llvm::StringRef member_name,
                     llvm::StringRef record_name) {
  const MemberExpr *member = member_expression(expression);
  return member != nullptr &&
         member->getMemberNameInfo().getAsString() == member_name &&
         member->getBase()->getType().getAsString().find(record_name) !=
             std::string::npos;
}

bool is_btech_dynamic_registration(const std::string &path,
                                   const CallExpr *push, const CallExpr *set) {
  if (path != "src/mux/lua/packages/btech/btech_package.c" ||
      called_name(set) != "lua_setfield" || push->getNumArgs() < 2 ||
      set->getNumArgs() < 3)
    return false;
  const FunctionDecl *handler = function_expression(push->getArg(1));
  return handler != nullptr && handler->getName() == "btech_lua_invoke" &&
         is_named_member(set->getArg(2), "name", "BtechLuaEntry");
}

bool is_internal_runtime_registration(const std::string &path,
                                      const CallExpr *push,
                                      const CallExpr *set) {
  if (path != "src/mux/lua/lua_runtime.c" ||
      called_name(set) != "lua_setfield" || push->getNumArgs() < 2 ||
      set->getNumArgs() < 3)
    return false;
  const FunctionDecl *handler = function_expression(push->getArg(1));
  const StringLiteral *leaf = string_expression(set->getArg(2));
  return handler != nullptr && handler->getName() == "lua_require_module" &&
         leaf != nullptr && leaf->getString() == "require";
}

bool is_test_traceback_callback(const std::string &path, const CallExpr *push,
                                const CallExpr *next) {
  if (path != "src/mux/lua/lua_test_runner.c" || push->getNumArgs() < 2 ||
      called_name(next) != "lua_insert")
    return false;
  const FunctionDecl *handler = function_expression(push->getArg(1));
  return handler != nullptr &&
         handler->getName() == "lua_test_traceback_handler";
}

const InitListExpr *initializer_list(const Expr *expression) {
  expression = strip_expression(expression);
  const auto *list = llvm::dyn_cast_or_null<InitListExpr>(expression);
  if (list != nullptr && list->getSyntacticForm() != nullptr)
    return list->getSyntacticForm();
  return list;
}

const CallExpr *call_statement(const Stmt *statement) {
  const auto *expression = llvm::dyn_cast_or_null<Expr>(statement);
  return llvm::dyn_cast_or_null<CallExpr>(strip_expression(expression));
}

bool uppercase_catalog_name(std::string_view value) {
  if (value.empty())
    return false;
  for (const char character : value) {
    if (!(character == '_' ||
          std::isdigit(static_cast<unsigned char>(character)) ||
          (character >= 'A' && character <= 'Z')))
      return false;
  }
  return true;
}

std::string ascii_upper(std::string value) {
  for (char &character : value) {
    if (character >= 'a' && character <= 'z')
      character = static_cast<char>(character - ('a' - 'A'));
  }
  return value;
}

void collect_string_literals(const Stmt *statement,
                             std::vector<std::string> &values) {
  if (statement == nullptr)
    return;
  if (const auto *literal = llvm::dyn_cast<StringLiteral>(statement))
    values.push_back(literal->getString().str());
  for (const Stmt *child : statement->children())
    collect_string_literals(child, values);
}

class ContractCommentHandler final : public clang::CommentHandler {
public:
  ContractCommentHandler(Model &model, const SourceManager &sources,
                         const clang::LangOptions &language)
      : model_(model), sources_(sources), language_(language) {}

  bool HandleComment(clang::Preprocessor &, clang::SourceRange range) override {
    const SourceLocation begin = sources_.getSpellingLoc(range.getBegin());
    if (!begin.isValid())
      return false;
    const std::string path = normalized_path(model_, sources_, begin);
    if (!is_project_source(path))
      return false;
    const llvm::StringRef text = clang::Lexer::getSourceText(
        clang::CharSourceRange::getTokenRange(range), sources_, language_);
    if (!text.contains("@par LuaLS"))
      return false;
    size_t marker = text.find("@par LuaLS");
    while (marker != llvm::StringRef::npos) {
      model_.visited_contract_markers.insert(contract_marker_identity(
          path, sources_.getFileOffset(begin) + marker));
      marker = text.find("@par LuaLS", marker + 10);
    }
    const std::string identity =
        path + ":" + std::to_string(sources_.getFileOffset(begin));
    model_.contract_comment_locations.emplace(
        identity, source_location(model_, sources_, begin));
    return false;
  }

private:
  Model &model_;
  const SourceManager &sources_;
  const clang::LangOptions &language_;
};

class CollectorVisitor final
    : public clang::RecursiveASTVisitor<CollectorVisitor> {
public:
  CollectorVisitor(ASTContext &context, Model &model)
      : context_(context), sources_(context.getSourceManager()), model_(model) {
  }

  bool VisitDecl(Decl *declaration) {
    if (declaration->isImplicit() || !declaration->getLocation().isValid())
      return true;
    const Location location =
        source_location(model_, sources_, declaration->getLocation());
    if (!is_project_source(location.path))
      return true;
    const RawComment *comment =
        context_.getRawCommentForDeclNoCache(declaration);
    if (comment == nullptr)
      return true;
    const SourceLocation begin =
        sources_.getSpellingLoc(comment->getBeginLoc());
    const std::string comment_path = normalized_path(model_, sources_, begin);
    if (!is_project_source(comment_path))
      return true;
    const std::string comment_identity =
        comment_path + ":" + std::to_string(sources_.getFileOffset(begin));
    if (!model_.seen_comments.insert(comment_identity).second)
      return true;
    parse_contract_comment(
        model_, comment->getRawText(sources_).str(),
        declaration_usr(declaration, sources_, model_),
        source_location(model_, sources_, comment->getBeginLoc()));
    return true;
  }

  bool VisitVarDecl(VarDecl *declaration) {
    if (!written_in_main_file(declaration->getLocation()) ||
        !declaration->isThisDeclarationADefinition() ||
        declaration->getInit() == nullptr)
      return true;
    const std::string name = declaration->getNameAsString();
    const std::string type = declaration->getType().getAsString();
    if (declaration->getType()->isArrayType() &&
        type.find("BtechLuaEntry") != std::string::npos)
      collect_btech_entries(*declaration);
    if (name == "LUA_ERROR_CODE_NAMES")
      collect_error_codes(*declaration);
    else if (name == "LUA_LOCK_DEFINITIONS")
      collect_array_catalog(*declaration, "mux.world.locks", 2, false);
    else if (name == "LUA_COMMAND_ACCESS_ENTRIES")
      collect_array_catalog(*declaration, "mux.world.access", 1, false);
    else if (name == "FLAG_ENTRIES")
      collect_array_catalog(*declaration, "mux.world.flags", 0, false);
    else if (name == "POWER_ENTRIES")
      collect_array_catalog(*declaration, "mux.world.powers", 0, true);
    else if (name == "CHANNEL_FLAGS")
      collect_array_catalog(*declaration, "mux.comsys.flags", 1, false);
    if (declaration->getType()->isArrayType() &&
        type.find("LuaMuxChannelMethod") != std::string::npos)
      collect_mux_method_table(*declaration);
    return true;
  }

  bool VisitFunctionDecl(FunctionDecl *declaration) {
    if (!written_in_main_file(declaration->getLocation()) ||
        !declaration->doesThisDeclarationHaveABody() ||
        declaration->getName() != "lua_mux_object_type_namespace_index")
      return true;
    std::vector<std::string> literals;
    collect_string_literals(declaration->getBody(), literals);
    std::set<std::string> values;
    for (const std::string &literal : literals) {
      if (uppercase_catalog_name(literal))
        values.insert(literal);
    }
    add_catalog("mux.world.types", *declaration, std::move(values));
    return true;
  }

  bool VisitStmt(Stmt *statement) {
    auto *compound = llvm::dyn_cast<CompoundStmt>(statement);
    if (compound == nullptr)
      return true;
    if (!written_in_main_file(compound->getBeginLoc()))
      return true;
    const Location location = source_location(
        model_, sources_, sources_.getExpansionLoc(compound->getBeginLoc()));
    const bool registration_source = is_mux_registration_source(location.path);
    std::vector<const Stmt *> statements(compound->body_begin(),
                                         compound->body_end());
    for (size_t index = 0; index < statements.size(); index++) {
      const CallExpr *push = call_statement(statements[index]);
      if (called_name(push) != "lua_pushcclosure" || push->getNumArgs() < 2)
        continue;
      disposed_callback_calls_.insert(push);
      const Location push_location = source_location(
          model_, sources_, sources_.getExpansionLoc(push->getExprLoc()));
      const CallExpr *set = index + 1 < statements.size()
                                ? call_statement(statements[index + 1])
                                : nullptr;
      if (!registration_source) {
        if (is_btech_dynamic_registration(location.path, push, set) ||
            is_internal_runtime_registration(location.path, push, set) ||
            is_test_traceback_callback(location.path, push, set))
          continue;
        model_.diagnose(
            push_location,
            "Lua C callback appears outside a recognized source scope");
        continue;
      }
      if (called_name(set) != "lua_setfield" || set->getNumArgs() < 3) {
        model_.diagnose(push_location,
                        "unrecognized MUX lua_pushcclosure registration");
        continue;
      }
      const StringLiteral *leaf_literal = string_expression(set->getArg(2));
      const Expr *handler_expression = push->getArg(1);
      if (const FunctionDecl *handler =
              function_expression(handler_expression)) {
        if (leaf_literal == nullptr) {
          model_.diagnose(push_location,
                          "MUX registration leaf is not a string literal");
          continue;
        }
        model_.registrations.push_back(
            {.module = "mux",
             .handler = declaration_usr(handler, sources_, model_),
             .leaf = leaf_literal->getString().str(),
             .location = push_location});
        continue;
      }
      if (const ParmVarDecl *parameter =
              parameter_expression(handler_expression)) {
        if (leaf_literal == nullptr) {
          model_.diagnose(push_location,
                          "MUX installer leaf is not a string literal");
          continue;
        }
        const auto *installer =
            llvm::dyn_cast_or_null<FunctionDecl>(parameter->getDeclContext());
        if (installer == nullptr) {
          model_.diagnose(push_location,
                          "MUX installer parameter has no function owner");
          continue;
        }
        model_.deferred_registrations.push_back(
            {.installer = declaration_usr(installer, sources_, model_),
             .parameter = parameter->getFunctionScopeIndex(),
             .leaf = leaf_literal->getString().str(),
             .location = push_location});
        continue;
      }
      if (has_mux_method_table_ &&
          is_channel_method_member(handler_expression, "function") &&
          is_channel_method_member(set->getArg(2), "name"))
        continue;
      model_.diagnose(push_location,
                      "MUX registration handler could not be resolved");
    }
    return true;
  }

  bool VisitCallExpr(CallExpr *call) {
    if (!written_in_main_file(call->getExprLoc()))
      return true;
    const FunctionDecl *callee = call->getDirectCallee();
    if (callee == nullptr)
      return true;
    if (callee->getName() == "lua_pushcclosure" &&
        !disposed_callback_calls_.contains(call)) {
      model_.diagnose(
          source_location(model_, sources_,
                          sources_.getExpansionLoc(call->getExprLoc())),
          "unsupported Lua C callback registration pattern");
    }
    if (callee->getName() == "lua_btech_install_bindings")
      collect_btech_installation(*call);
    InstallerCall invocation;
    invocation.installer = declaration_usr(callee, sources_, model_);
    invocation.location = source_location(model_, sources_, call->getExprLoc());
    invocation.argument_functions.reserve(call->getNumArgs());
    for (const Expr *argument : call->arguments()) {
      const FunctionDecl *function = function_expression(argument);
      invocation.argument_functions.push_back(
          function == nullptr ? std::string()
                              : declaration_usr(function, sources_, model_));
    }
    model_.installer_calls.push_back(std::move(invocation));
    return true;
  }

private:
  bool written_in_main_file(SourceLocation location) const {
    return sources_.isWrittenInMainFile(sources_.getExpansionLoc(location));
  }

  void add_catalog(const std::string &name, const Decl &owner,
                   std::set<std::string> values) {
    CatalogFact fact = {
        .owner = declaration_usr(&owner, sources_, model_),
        .values = std::move(values),
        .location = source_location(model_, sources_, owner.getLocation()),
    };
    const auto [iterator, inserted] = model_.catalogs.emplace(name, fact);
    if (!inserted && (iterator->second.owner != fact.owner ||
                      iterator->second.values != fact.values))
      model_.diagnose(fact.location,
                      "native Lua catalog was collected inconsistently");
  }

  void collect_error_codes(const VarDecl &declaration) {
    const InitListExpr *outer = initializer_list(declaration.getInit());
    if (outer == nullptr) {
      model_.diagnose(
          source_location(model_, sources_, declaration.getLocation()),
          "native Lua error catalog is not an initializer list");
      return;
    }
    std::set<std::string> mux;
    std::set<std::string> btech;
    std::set<std::string> testing;
    for (const Expr *element : outer->inits()) {
      std::vector<std::string> literals;
      collect_string_literals(element, literals);
      if (literals.size() != 1) {
        model_.diagnose(
            source_location(model_, sources_, element->getExprLoc()),
            "native Lua error catalog entry is not one string "
            "literal");
        continue;
      }
      const std::string &literal = literals.front();
      std::set<std::string> *destination = nullptr;
      if (literal.starts_with("mux."))
        destination = &mux;
      else if (literal.starts_with("btech."))
        destination = &btech;
      else if (literal.starts_with("testing."))
        destination = &testing;
      else {
        model_.diagnose(
            source_location(model_, sources_, element->getExprLoc()),
            "native Lua error catalog entry has an unsupported "
            "root: " +
                literal);
        continue;
      }
      if (!destination->insert(literal).second)
        model_.diagnose(
            source_location(model_, sources_, element->getExprLoc()),
            "native Lua error catalog contains a duplicate value: " + literal);
    }
    add_catalog("mux.error.codes", declaration, std::move(mux));
    add_catalog("btech.error.codes", declaration, std::move(btech));
    add_catalog("mux.testing.codes", declaration, std::move(testing));
  }

  void collect_array_catalog(const VarDecl &declaration,
                             const std::string &catalog, unsigned field,
                             bool uppercase) {
    const InitListExpr *outer = initializer_list(declaration.getInit());
    if (outer == nullptr) {
      model_.diagnose(
          source_location(model_, sources_, declaration.getLocation()),
          "native Lua catalog is not an initializer list");
      return;
    }
    std::set<std::string> values;
    const auto elements = outer->inits();
    for (size_t index = 0; index < elements.size(); index++) {
      const Expr *element = elements[index];
      const InitListExpr *row = initializer_list(element);
      const Location location =
          source_location(model_, sources_, element->getExprLoc());
      if (row == nullptr || field >= row->getNumInits()) {
        model_.diagnose(location, "malformed native Lua catalog row");
        continue;
      }
      const Expr *field_expression = row->getInit(field);
      const StringLiteral *literal = string_expression(field_expression);
      if (literal == nullptr) {
        if (index + 1 == elements.size() &&
            field_expression->isNullPointerConstant(
                context_, Expr::NPC_ValueDependentIsNotNull))
          continue;
        model_.diagnose(location,
                        "native Lua catalog row field is not a string literal");
        continue;
      }
      std::string value = literal->getString().str();
      if (uppercase)
        value = ascii_upper(std::move(value));
      if (!values.insert(value).second)
        model_.diagnose(location,
                        "native Lua catalog contains a duplicate value: " +
                            value);
    }
    add_catalog(catalog, declaration, std::move(values));
  }

  void collect_btech_entries(const VarDecl &declaration) {
    const std::string inventory =
        declaration_usr(&declaration, sources_, model_);
    model_.btech_inventories.emplace(
        inventory,
        source_location(model_, sources_, declaration.getLocation()));
    const InitListExpr *outer = initializer_list(declaration.getInit());
    if (outer == nullptr) {
      model_.diagnose(
          source_location(model_, sources_, declaration.getLocation()),
          "BtechLuaEntry inventory is not an initializer list");
      return;
    }
    for (const Expr *element : outer->inits()) {
      const InitListExpr *row = initializer_list(element);
      const Location location =
          source_location(model_, sources_, element->getExprLoc());
      if (row == nullptr || row->getNumInits() < 3) {
        model_.diagnose(location, "malformed BtechLuaEntry initializer");
        continue;
      }
      const StringLiteral *name = string_expression(row->getInit(0));
      const StringLiteral *qualified = string_expression(row->getInit(1));
      const FunctionDecl *handler = function_expression(row->getInit(2));
      if (name == nullptr || qualified == nullptr || handler == nullptr) {
        model_.diagnose(location,
                        "could not resolve BtechLuaEntry initializer");
        continue;
      }
      model_.btech_entries.push_back(
          {.name = name->getString().str(),
           .qualified_name = qualified->getString().str(),
           .handler = declaration_usr(handler, sources_, model_),
           .inventory = inventory,
           .location = location});
    }
  }

  void collect_btech_installation(const CallExpr &call) {
    const Location location =
        source_location(model_, sources_, call.getExprLoc());
    if (call.getNumArgs() < 4) {
      model_.diagnose(location,
                      "lua_btech_install_bindings call has too few arguments");
      return;
    }
    const StringLiteral *package = string_expression(call.getArg(2));
    const VarDecl *inventory = variable_expression(call.getArg(3));
    if (package == nullptr) {
      model_.diagnose(location,
                      "BTech installer package is not a string literal");
      return;
    }
    if (inventory == nullptr || !inventory->getType()->isArrayType() ||
        inventory->getType().getAsString().find("BtechLuaEntry") ==
            std::string::npos) {
      model_.diagnose(location,
                      "BTech installer inventory could not be resolved");
      return;
    }
    model_.btech_installations.push_back(
        {.inventory = declaration_usr(inventory, sources_, model_),
         .package = package->getString().str(),
         .location = location});
  }

  void collect_mux_method_table(const VarDecl &declaration) {
    const InitListExpr *outer = initializer_list(declaration.getInit());
    if (outer == nullptr) {
      model_.diagnose(
          source_location(model_, sources_, declaration.getLocation()),
          "LuaMuxChannelMethod inventory is not an initializer list");
      return;
    }
    for (const Expr *element : outer->inits()) {
      const InitListExpr *row = initializer_list(element);
      if (row == nullptr || row->getNumInits() < 2) {
        model_.diagnose(
            source_location(model_, sources_, element->getExprLoc()),
            "malformed LuaMuxChannelMethod initializer");
        continue;
      }
      const StringLiteral *leaf = string_expression(row->getInit(0));
      const FunctionDecl *handler = function_expression(row->getInit(1));
      if (leaf == nullptr || handler == nullptr) {
        model_.diagnose(
            source_location(model_, sources_, element->getExprLoc()),
            "could not resolve LuaMuxChannelMethod initializer");
        continue;
      }
      model_.registrations.push_back(
          {.module = "mux",
           .handler = declaration_usr(handler, sources_, model_),
           .leaf = leaf->getString().str(),
           .location =
               source_location(model_, sources_, element->getExprLoc())});
      has_mux_method_table_ = true;
    }
  }

  ASTContext &context_;
  SourceManager &sources_;
  Model &model_;
  bool has_mux_method_table_ = false;
  std::set<const CallExpr *> disposed_callback_calls_;
};

class CollectorConsumer final : public clang::ASTConsumer {
public:
  CollectorConsumer(ASTContext &context, Model &model)
      : visitor_(context, model) {}

  void HandleTranslationUnit(ASTContext &context) override {
    visitor_.TraverseDecl(context.getTranslationUnitDecl());
  }

private:
  CollectorVisitor visitor_;
};

class CollectorAction final : public clang::ASTFrontendAction {
public:
  explicit CollectorAction(Model &model) : model_(model) {}

  bool BeginSourceFileAction(clang::CompilerInstance &compiler) override {
    comment_handler_ = std::make_unique<ContractCommentHandler>(
        model_, compiler.getSourceManager(), compiler.getLangOpts());
    compiler.getPreprocessor().addCommentHandler(comment_handler_.get());
    return true;
  }

  std::unique_ptr<clang::ASTConsumer>
  CreateASTConsumer(clang::CompilerInstance &compiler,
                    llvm::StringRef) override {
    return std::make_unique<CollectorConsumer>(compiler.getASTContext(),
                                               model_);
  }

  void EndSourceFileAction() override {
    getCompilerInstance().getPreprocessor().removeCommentHandler(
        comment_handler_.get());
    comment_handler_.reset();
  }

private:
  Model &model_;
  std::unique_ptr<ContractCommentHandler> comment_handler_;
};

class CollectorFactory final : public clang::tooling::FrontendActionFactory {
public:
  explicit CollectorFactory(Model &model) : model_(model) {}

  std::unique_ptr<clang::FrontendAction> create() override {
    return std::make_unique<CollectorAction>(model_);
  }

private:
  Model &model_;
};

} // namespace

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_collector_factory(Model &model) {
  return std::make_unique<CollectorFactory>(model);
}

} // namespace lua_types
