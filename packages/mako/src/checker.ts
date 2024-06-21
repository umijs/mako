import fs from 'fs';
import path from 'path';
import chalk from 'chalk';
import semver from 'semver';

let BLACKLIST_PACKAGES: any = {
  'pdfjs-dist': {
    version: '<3.0.0',
    message:
      '`pdfjs-dist@2` is not supported, please use `pdfjs-dist@3` or above instead.',
  },
  'monaco-editor': {
    version: '*',
    message:
      '`monaco-editor` is not supported, please use `@monaco-editor/react` instead.',
  },
};

export function check(root: string) {
  let pkgPath = path.join(root, 'package.json');
  if (fs.existsSync(pkgPath)) {
    let pkg = require(pkgPath);
    let bPkgs = Object.keys(BLACKLIST_PACKAGES);
    for (let name of bPkgs) {
      if (!depExists(pkg, name)) continue;
      let version = getDepVersion(root, name);
      if (
        version &&
        semver.satisfies(version, BLACKLIST_PACKAGES[name].version)
      ) {
        console.error(chalk.red(`Error: ${BLACKLIST_PACKAGES[name].message}`));
        process.exit(1);
      }
    }
  }
}

function depExists(pkg: any, name: string) {
  return pkg.dependencies?.[name] || pkg.devDependencies?.[name];
}

function getDepVersion(root: string, name: string) {
  let pkgPath = path.join(root, 'node_modules', name, 'package.json');
  if (fs.existsSync(pkgPath)) {
    let pkg = require(pkgPath);
    return pkg.version;
  }
}
