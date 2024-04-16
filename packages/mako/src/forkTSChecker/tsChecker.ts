import ts from 'typescript';
import path from 'path';
import { promises as fs } from 'fs';

class TypeChecker {
  #projectRoot: string;
  constructor(projectRoot: string) {
    this.#projectRoot = projectRoot;
  }

  async check() {
    try {
      const configPath = ts.findConfigFile(
        this.#projectRoot,
        ts.sys.fileExists,
        'tsconfig.json',
      );
      if (!configPath) {
        console.error(
          'Could not find a valid "tsconfig.json" file in the project root:',
          this.#projectRoot,
        );
        return;
      }
      let configFileText = '';
      try {
        configFileText = await fs.readFile(configPath, 'utf8');
      } catch (readError) {
        console.error(
          `Error reading the "tsconfig.json" file at ${configPath}:`,
          readError,
        );
        return;
      }
      const configFile = ts.parseConfigFileTextToJson(
        configPath,
        configFileText,
      );
      if (configFile.error) {
        console.error('Error parsing "tsconfig.json" file:', configFile.error);
        return;
      }
      let parsedCommandLine;
      try {
        parsedCommandLine = ts.parseJsonConfigFileContent(
          configFile.config,
          ts.sys,
          path.dirname(configPath),
        );
      } catch (parseError) {
        console.error(
          'Error parsing the configuration from "tsconfig.json":',
          parseError,
        );
        return;
      }
      let program;
      try {
        program = ts.createProgram({
          rootNames: parsedCommandLine.fileNames,
          options: { ...parsedCommandLine.options, noEmit: true },
        });
      } catch (programError) {
        console.error('Error creating the TypeScript program:', programError);
        return;
      }
      const diagnostics = ts.getPreEmitDiagnostics(program);
      if (diagnostics.length > 0) {
        diagnostics.forEach((diagnostic: any) => {
          const message = ts.flattenDiagnosticMessageText(
            diagnostic.messageText,
            '\n',
          );
          if (diagnostic.file && typeof diagnostic.start === 'number') {
            const { line, character } =
              diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
            console.error(
              `${diagnostic.file.fileName} (${line + 1}, ${character + 1
              }): ${message}`,
            );
          } else {
            console.error(message);
          }
        });
      } else {
        console.log('No type errors found.');
      }
    } catch (error) {
      console.error(
        'An unexpected error occurred during type checking:',
        error,
      );
    }
  }
}

module.exports = { TypeChecker };
