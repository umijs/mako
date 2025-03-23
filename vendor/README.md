# Vendor

This directory contains scripts and templates for publishing npm packages.

## Package Structure

We publish two types of packages for each binary:

1. Main Package (`@utoo/<binary-name>`)
   - Platform-independent entry package
   - Contains postinstall script to download appropriate binary package
   - Serves as the main entry point for users

2. Binary Package (`@utoo/<binary-name>-<os>-<cpu>`)
   - Platform-specific package containing the actual binary
   - Published for each supported platform (darwin-x64, darwin-arm64, linux-x64)
   - Automatically downloaded by the main package during installation

## Publishing Process

The publishing process is automated through GitHub Actions:

1. Binary Packages:
   - Builds binaries for each platform (darwin-x64, darwin-arm64, linux-x64)
   - Uses `npm-binary.sh` to package and publish platform-specific binaries
   - Each binary package contains the actual executable

2. Main Packages:
   - Published after all binary packages are available
   - Uses `npm-main.sh` to create the entry package
   - Contains a postinstall script that:
     - Detects user's platform
     - Downloads the appropriate binary package
     - Sets up the binary in the user's environment

## Supported Platforms

- macOS (x64, arm64)
- Linux (x64)

## Scripts

- `npm-binary.sh`: Packages and publishes platform-specific binaries
- `npm-main.sh`: Creates and publishes the main entry package
- `postinstall.sh`: Handles binary installation after package installation

## Templates

- `binary.package.json.template`: Template for binary package configuration
- `entry.package.json.template`: Template for main package configuration
- `postinstall.sh.template`: Template for binary installation script
