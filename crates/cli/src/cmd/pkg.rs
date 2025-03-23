// use crate::util::logger::log_info;
// use clap::{App, ArgMatches, SubCommand};
// use serde_json::Value;
// use std::fs;
// use std::process::{Command, Stdio};

// pub struct PackageJson {
//     scripts: Option<Value>,
// }

// impl PackageJson {
//     fn load() -> Option<Self> {
//         if let Ok(content) = fs::read_to_string("package.json") {
//             if let Ok(json) = serde_json::from_str::<Value>(&content) {
//                 return Some(PackageJson {
//                     scripts: json.get("scripts").cloned(),
//                 });
//             }
//         }
//         None
//     }

//     fn run_script(&self, script_name: &str) -> Result<(), String> {
//         if let Some(Value::Object(scripts)) = &self.scripts {
//             if scripts.contains_key(script_name) {
//                 log_info(&format!("Executing npm run {}", script_name));
//                 let status = Command::new("npm")
//                     .args(&["run", script_name])
//                     .stdout(Stdio::inherit())
//                     .stderr(Stdio::inherit())
//                     .status()
//                     .map_err(|e| e.to_string())?;

//                 if !status.success() {
//                     return Err(format!("npm run {} execution failed", script_name));
//                 }
//                 return Ok(());
//             }
//         }
//         Err(format!("Script not found: {}", script_name))
//     }
// }

// pub fn extend_app(mut app: App) -> (App, String) {
//     let pkg = PackageJson::load();
//     let mut scripts_help = String::new();

//     app = app.subcommand(
//         SubCommand::with_name("run")
//             .about(
//                 "Run scripts defined in package.json (e.g. utoo build, utoo run build) (alias: r)",
//             )
//             .alias("r")
//             .arg(
//                 clap::Arg::with_name("script")
//                     .help("Script name to run")
//                     .required(true),
//             ),
//     );

//     if let Some(pkg_ref) = &pkg {
//         if let Some(Value::Object(scripts)) = &pkg_ref.scripts {
//             let reserved_commands = ["install", "rebuild", "clean", "run", "deps"];

//             // Build script help information
//             scripts_help = {
//                 const GREEN: &str = "\x1b[32m";
//                 const YELLOW: &str = "\x1b[33m";
//                 const RESET: &str = "\x1b[0m";

//                 let mut help = format!("\n{}SCRIPTS (IN PACKAGE.JSON){}:\n", YELLOW, RESET);

//                 // Find the longest command name length
//                 let max_name_len = scripts
//                     .keys()
//                     .filter(|name| !reserved_commands.contains(&name.as_str()))
//                     .map(|name| name.len())
//                     .max()
//                     .unwrap_or(0);

//                 const MAX_DESC_LEN: usize = 50;

//                 for (name, cmd) in scripts {
//                     if !reserved_commands.contains(&name.as_str()) {
//                         if let Value::String(cmd_str) = cmd {
//                             let desc = if cmd_str.len() > MAX_DESC_LEN {
//                                 format!("{}...", &cmd_str[..MAX_DESC_LEN - 3])
//                             } else {
//                                 cmd_str.to_string()
//                             };

//                             // Separate name handling and padding
//                             let colored_name = format!("{}{}", GREEN, name);
//                             let padding = " ".repeat(max_name_len - name.len());

//                             help.push_str(&format!(
//                                 "    {}{}{}{}\n",
//                                 colored_name,
//                                 padding,
//                                 RESET,
//                                 format!("    {}", desc)
//                             ));
//                         }
//                         let sub = SubCommand::with_name(name)
//                             .about("Run npm script")
//                             .hide(true);
//                         app = app.subcommand(sub);
//                     }
//                 }
//                 help
//             };
//         }
//     }

//     (app, scripts_help)
// }

// pub fn handle_command(matches: &ArgMatches) -> Option<Result<(), String>> {
//     let pkg = PackageJson::load()?;

//     let cmd_name = matches.subcommand_name()?;

//     // Skip built-in commands
//     if ["install", "rebuild", "clean", "deps"].contains(&cmd_name) {
//         return None;
//     }

//     match cmd_name {
//         "run" => {
//             let run_matches = matches.subcommand_matches("run")?;
//             let script = run_matches.value_of("script").unwrap();
//             Some(pkg.run_script(script))
//         }
//         script_name => {
//             if let Some(Value::Object(scripts)) = &pkg.scripts {
//                 if scripts.contains_key(script_name) {
//                     Some(pkg.run_script(script_name))
//                 } else {
//                     None
//                 }
//             } else {
//                 None
//             }
//         }
//     }
// }
