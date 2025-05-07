# Utoo

Utoo is a modern frontend toolchain that provides a unified command-line interface for frontend development. It comes with built-in package management capabilities and can be extended with additional tools like the bundler.

- `ut`: The core toolchain entry point, providing command mounting and configuration management
- `utoo`: Built-in package manager for dependency resolution and installation
- `@utoo/bundler`: Optional high-performance bundler (requires separate installation)

## 🚀 Installation

### 📦 Core Tools

Install the core tools (`ut` and `utoo`) globally:

```bash
npm install -g utoo
```

### 🛠️ Bundler (Optional)

Install the bundler globally if you need build capabilities:

```bash
npm install -g @utoo/bundler
```

## ✨ Features

- 🚀 Fast package management
- 🔧 Flexible configuration system
- 📦 Smart dependency resolution
- 🛠️ Powerful build toolchain

## 🔌 Command Mounting

Utoo provides a powerful command mounting system through the `ut` command. This system allows you to:

1. **Mount Custom Commands**: Map any command to a custom alias
2. **Configure Global/Local Commands**: Set commands at global or project level
3. **Use Wildcard Commands**: Define default behavior for unknown commands

### ⚙️ Configuration

Commands can be configured using the `ut config` command:

```bash
# Set a global command
ut config set install.cmd "utoo install" --global

# Set a local command (project-specific)
ut config set build.cmd "utoo-bundler build"

# Set a wildcard command (default behavior)
ut config set *.cmd "utoo" --global
```

### 🔍 Command Resolution

When you run a command through `ut`, it follows this resolution order:

1. Check for a specific command configuration
2. If not found, check for a wildcard command
3. If no wildcard is configured, default to `utoo`

For example:
```bash
# These commands are equivalent if configured as shown above
ut install
utoo install

# Custom command mapping
ut build
# Executes: utoo-bundler build

# Unknown command with wildcard
ut unknown-command
# Executes: utoo unknown-command
```

### 📁 Configuration Files

Command configurations are stored in:

- Global config: `~/.utoo/config.toml`
- Local config: `.utoo.toml` (project root)

Example configuration:
```toml
[values]
"install.cmd" = "utoo install"
"build.cmd" = "utoo-bundler build"
"*.cmd" = "utoo"
```

### 📋 Available Commands

```bash
# Show help and available commands
ut --help

# List all configured commands
ut config list

# Get specific command configuration
ut config get install.cmd

# Set command configuration
ut config set install.cmd "utoo install" --global
```

## 🔌 Command Mounting

Utoo provides a powerful command mounting system that allows you to extend the toolchain with custom commands. This is particularly useful for project-specific scripts and workflows.

### 📜 Package.json Scripts

Any script defined in your `package.json` can be executed directly through Utoo:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "jest"
  }
}
```

You can run these scripts using:

```bash
# Using the run command
utoo run dev
# or shorthand
utoo r dev

# Direct command execution
utoo dev
utoo build
utoo test
```

### 🛠️ Custom Commands

You can create custom commands by adding them to your project's configuration. These commands can be:

1. **Shell Scripts**: Simple shell commands or scripts
2. **Node.js Scripts**: JavaScript/TypeScript files
3. **Binary Executables**: Any executable in your project

### 🌍 Command Execution Environment

When executing commands, Utoo provides:

- Access to all project dependencies
- Environment variables from your project
- Proper working directory context
- Node.js binary path resolution

### ⚡ Command Hooks

Utoo supports various command hooks that can be used to extend command behavior:

- `preinstall`: Run before package installation
- `install`: Run during package installation
- `postinstall`: Run after package installation
- `prepare`: Run before package preparation
- `preprepare`: Run before package preparation
- `postprepare`: Run after package preparation

## 📋 Commands

### 📦 Package Management Commands

#### 📥 Install Dependencies
```bash
# Install project dependencies
utoo install
# or shorthand
utoo i

# Install specific package
utoo install <package-name>
# Example: install lodash
utoo install lodash

# Install as dev dependency
utoo install <package-name> --save-dev

# Install as peer dependency
utoo install <package-name> --save-peer

# Install as optional dependency
utoo install <package-name> --save-optional

# Global installation
utoo install <package-name> -g
```

#### 🗑️ Uninstall Dependencies
```bash
# Uninstall specific package
utoo uninstall <package-name>
# or shorthand
utoo un <package-name>
```

#### 🔄 Update Dependencies
```bash
# Update all dependencies
utoo update
# or shorthand
utoo u
```

### 🏗️ Build Commands

#### 🔨 Rebuild Dependencies
```bash
# Rebuild all dependencies
utoo rebuild
# or shorthand
utoo rb
```

#### 🧹 Clean Cache
```bash
# Clean all cache
utoo clean

# Clean specific package cache
utoo clean <package-pattern>
# Example: clean all react related packages
utoo clean "react*"
```

#### 📊 Dependency Analysis
```bash
# Analyze project dependencies
utoo deps
# or shorthand
utoo d

# Only analyze workspace dependencies
utoo deps --workspace-only
```

### 🛠️ Bundler

Utoo includes a high-performance bundler that supports various build scenarios:

#### 🚀 Basic Usage
```bash
# Start development server
utoo-bundler dev

# Build production version
utoo-bundler build
```

#### 📋 Supported Build Scenarios

1. **Basic Application Build**
   - TypeScript/JavaScript support
   - CSS/SCSS/Less support
   - Asset file processing

2. **Library Mode Build**
   - Multi-entry build support
   - Tree Shaking
   - External dependency configuration

3. **Style Processing**
   - CSS Modules
   - Sass/SCSS
   - Less
   - PostCSS

4. **Development Features**
   - Hot Module Replacement (HMR)
   - Source Maps
   - Fast Refresh

#### 📚 Example Projects

We provide several example projects to demonstrate different usage scenarios:

- `examples/with-antd`: Ant Design component library integration
- `examples/with-sass`: Sass style processing
- `examples/with-less`: Less style processing
- `examples/with-style-loader`: CSS Modules usage
- `examples/with-library`: Library mode build

### ⚙️ Common Options

All commands support the following common options:

- `--verbose`: Show detailed output
- `--registry <url>`: Set npm registry URL
- `--legacy-peer-deps`: Use legacy peer dependency handling
- `--ignore-scripts`: Skip running dependency scripts

## 🚀 Quick Start

### 🔨 Build from Source

```bash
# Build project
cargo build --release

# Add binary to PATH
export PATH=$PATH:$(pwd)/target/release
```

### 📦 Install Dependencies

```bash
# Install project dependencies using utoo
ut
```

### 🏃 Run Bundler

```bash
# Build local development environment
cd packages/bundler
ut build
```

## 📁 Project Structure

```
.
├── crates/           # Rust core libraries
│   ├── cli/         # Command line tools
│   ├── core/        # Core functionality
│   ├── bundler-*    # Build related modules
├── packages/         # Package management code
├── examples/         # Example projects
└── vendor/          # Third-party dependencies
```
