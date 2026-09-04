#include <algorithm>
#include <cctype>
#include <filesystem>
#include <map>
#include <memory>
#include <string>
#include <tuple>
#include <vector>

#include "clang/Tooling/CommonOptionsParser.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "clang/Tooling/Tooling.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/raw_ostream.h"

#include "ast_collector.h"
#include "contracts.h"
#include "model.h"
#include "source_selection.h"

namespace {

class SingleCommandCompilationDatabase final
    : public clang::tooling::CompilationDatabase {
public:
  explicit SingleCommandCompilationDatabase(
      const clang::tooling::CompilationDatabase &underlying)
      : underlying_(underlying) {}

  std::vector<clang::tooling::CompileCommand>
  getCompileCommands(llvm::StringRef file_path) const override {
    std::vector<clang::tooling::CompileCommand> commands =
        underlying_.getCompileCommands(file_path);
    if (commands.empty())
      return commands;
    std::sort(commands.begin(), commands.end(), command_less);
    commands.erase(commands.begin() + 1, commands.end());
    return commands;
  }

  std::vector<std::string> getAllFiles() const override {
    return underlying_.getAllFiles();
  }

private:
  static unsigned command_rank(const clang::tooling::CompileCommand &command) {
    if (command.Output.find("CMakeFiles/stompymux.dir/") != std::string::npos)
      return 0;
    std::string output = command.Output;
    std::transform(output.begin(), output.end(), output.begin(),
                   [](unsigned char character) {
                     return static_cast<char>(std::tolower(character));
                   });
    return output.find("test") == std::string::npos ? 1 : 2;
  }

  static bool command_less(const clang::tooling::CompileCommand &left,
                           const clang::tooling::CompileCommand &right) {
    return std::tuple(command_rank(left), left.Directory, left.Output,
                      left.CommandLine) <
           std::tuple(command_rank(right), right.Directory, right.Output,
                      right.CommandLine);
  }

  const clang::tooling::CompilationDatabase &underlying_;
};

llvm::cl::OptionCategory GENERATOR_CATEGORY("lua-type-generator options");
llvm::cl::opt<bool> CHECK("check", llvm::cl::desc("Check tracked outputs"),
                          llvm::cl::init(false),
                          llvm::cl::cat(GENERATOR_CATEGORY));
llvm::cl::opt<bool> WRITE("write", llvm::cl::desc("Write changed outputs"),
                          llvm::cl::init(false),
                          llvm::cl::cat(GENERATOR_CATEGORY));
llvm::cl::opt<std::string> REPOSITORY_ROOT("repo-root",
                                           llvm::cl::desc("Repository root"),
                                           llvm::cl::init("."),
                                           llvm::cl::value_desc("path"),
                                           llvm::cl::cat(GENERATOR_CATEGORY));
llvm::cl::opt<std::string> OUTPUT_DIRECTORY("output-dir",
                                            llvm::cl::desc("Output directory"),
                                            llvm::cl::init("game/lua/types"),
                                            llvm::cl::value_desc("path"),
                                            llvm::cl::cat(GENERATOR_CATEGORY));

} // namespace

int main(int argc, const char **argv) {
  auto options = clang::tooling::CommonOptionsParser::create(
      argc, argv, GENERATOR_CATEGORY, llvm::cl::OneOrMore);
  if (!options) {
    llvm::errs() << llvm::toString(options.takeError()) << '\n';
    return 2;
  }
  if (CHECK == WRITE) {
    llvm::errs() << "exactly one of --check and --write is required\n";
    return 2;
  }

  std::error_code root_error;
  std::filesystem::path root = std::filesystem::weakly_canonical(
      std::filesystem::path(REPOSITORY_ROOT.getValue()), root_error);
  if (root_error) {
    llvm::errs() << "cannot resolve repository root: " << root_error.message()
                 << '\n';
    return 2;
  }
  std::filesystem::path output(OUTPUT_DIRECTORY.getValue());
  if (output.is_relative())
    output = root / output;

  lua_types::Model model;
  model.repository_root = root.string();
  std::unique_ptr<clang::tooling::FrontendActionFactory> factory =
      lua_types::make_collector_factory(model);
  std::vector<std::string> sources = options->getSourcePathList();
  for (std::string &source : sources) {
    std::error_code source_error;
    const std::filesystem::path canonical =
        std::filesystem::weakly_canonical(source, source_error);
    if (!source_error)
      source = canonical.string();
  }
  std::sort(sources.begin(), sources.end());
  sources.erase(std::unique(sources.begin(), sources.end()), sources.end());
  const SingleCommandCompilationDatabase compilations(
      options->getCompilations());
  const std::vector<std::string> selected =
      lua_types::select_source_files(model, root, sources, compilations);
  for (const std::string &source : selected) {
    clang::tooling::ClangTool tool(compilations, {source});
    tool.setPrintErrorMessage(false);
    const int tool_status = tool.run(factory.get());
    if (tool_status != 0)
      return tool_status;
  }

  lua_types::validate_model(model);
  if (!model.diagnostics.empty()) {
    lua_types::print_diagnostics(model);
    return 1;
  }
  const std::map<std::string, std::string> outputs =
      lua_types::render_modules(model);
  const bool succeeded = CHECK
                             ? lua_types::check_outputs(model, outputs, output)
                             : lua_types::write_outputs(model, outputs, output);
  if (!model.diagnostics.empty())
    lua_types::print_diagnostics(model);
  return succeeded && model.diagnostics.empty() ? 0 : 1;
}
