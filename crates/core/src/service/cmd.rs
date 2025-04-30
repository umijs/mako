use crate::config::Config;
use anyhow::Result;
use colored::*;
use std::process::Command;

pub struct CommandService {
    config: Config,
}

impl CommandService {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn get_available_commands(&self) -> Result<Vec<(String, String)>> {
        let mut commands = Vec::new();

        // Get all config keys
        for (key, _) in self.config.list()? {
            if key.ends_with(".cmd") {
                // Remove .cmd suffix to get command name
                if let Some(command) = key.strip_suffix(".cmd") {
                    // Get the command alias
                    if let Some(alias) = self.config.get(&format!("{}.cmd", command))? {
                        commands.push((command.to_string(), alias));
                    }
                }
            }
        }

        Ok(commands)
    }

    pub fn print_help(&self) -> Result<()> {
        let commands = self.get_available_commands()?;

        println!("{}", "ut - A command line tool".bold());
        println!();
        println!("{}", "Usage:".bold());
        println!("  ut <COMMAND>");
        println!();
        println!("{}", "Commands:".bold());

        // Find the longest command name
        let max_width = commands
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0)
            .max("config".len());

        println!("  {:<width$}    {}", "config".cyan(), "Manage configuration", width = max_width);
        for (name, alias) in commands {
            println!("  {:<width$}    {} {}", name.cyan(), "→".bold(), alias, width = max_width);
        }
        println!();
        println!("{}", "Options:".bold());
        println!("  {}     {}", "-h, --help".yellow(), "Print help information");
        println!("  {}  {}", "-V, --version".yellow(), "Print version information");
        println!();
        println!("For more information about a command, try 'ut <command> --help'");

        Ok(())
    }

    pub fn execute(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Ok(());
        }

        let command_name = &args[0];

        // First try to find specific command
        let (aliased_command, is_wildcard) = if let Some(cmd) = self.config.get(&format!("{}.cmd", command_name))? {
            (cmd, false)
        } else if let Some(cmd) = self.config.get("*.cmd")? {
            (cmd.replace("*", command_name), true)
        } else {
            println!("Command '{}' not found", command_name);
            std::process::exit(1);
        };

        // Split the aliased command into parts
        let mut parts: Vec<&str> = aliased_command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        // Get the command and its arguments
        let cmd = parts.remove(0);
        let mut command = Command::new(cmd);

        if is_wildcard {
            // For wildcard commands, forward all original arguments
            command.args(args);
        } else {
            // For specific commands, add the original arguments except the command name
            command.args(&args[1..]);
            // Add any additional arguments from the alias
            command.args(parts);
        }

        // Execute the command
        let status: std::process::ExitStatus = command.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
