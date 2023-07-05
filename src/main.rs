mod service_manager;
mod script;
mod service;
mod user_command;
mod server;
mod command_to_run;
mod utilities;

use std::fs;
use std::env::args;
use std::io::Error;
use std::process::exit;
use yaml_rust::YamlLoader;
use ctrlc;
use crate::server::{send_command_to_server, server_start};
use crate::service_manager::ServiceManager;
use crate::user_command::{run_user_commands, WriterWithTCP};
use crate::utilities::{build_invalid_data_error_str, build_invalid_data_error_string};

static mut MANAGER: Option<ServiceManager> = None;

fn usage() {
    println!("Usage: runner [config_file_name] [commands]")
}

fn main() -> Result<(), Error> {
    let mut config_file = None;
    let mut commands = Vec::new();
    let mut n = 0;
    let mut noinit = false;
    let mut noexec = false;
    for arg in args() {
        if n != 0 {
            if arg == "noinit" {
                noinit = true;
            } else if arg == "noexec" {
                noexec = true;
            } else if n == 1 && arg.ends_with(".yml") {
                config_file = Some(arg);
            } else {
                commands.push(arg);
            }
        }
        n += 1;
    }
    if config_file.is_none() && commands.len() == 0 {
        usage();
        return Ok(());
    }
    if let Some(config) = config_file {
        let contents = fs::read_to_string(config)?;
        let docs = YamlLoader::load_from_str(contents.as_str())
            .map_err(|e|build_invalid_data_error_string(e.to_string()))?;
        let doc = &docs[0];
        let services = doc["services"].as_hash()
            .ok_or(build_invalid_data_error_str("could not find any service"))?;
        let service_sets = doc["service-sets"].as_hash()
            .ok_or(build_invalid_data_error_str("could not find any service"))?;
        let init_command = if noinit { None } else { doc["init-command"].as_str().map(|s| s.to_string()) };
        let shutdown_command = if noinit { None } else { doc["shutdown-command"].as_str().map(|s| s.to_string()) };

        let manager = ServiceManager::new(service_sets, services, init_command,
                                          shutdown_command, noexec)?;
        unsafe {
            MANAGER = Some(manager);

            if commands.len() > 0 {
                run_user_commands(commands, MANAGER.as_ref().unwrap(), noexec, WriterWithTCP::new(None));
            }

            if noexec {
                ctrlc::set_handler(|| {shutdown(true, WriterWithTCP::new(None))})
            } else {
                ctrlc::set_handler(|| {shutdown(false, WriterWithTCP::new(None))})
            }.map_err(|e|build_invalid_data_error_string(e.to_string()))?;

            return server_start(MANAGER.as_ref().unwrap(), noexec);
        }
    }
    send_command_to_server(commands.join(" "))
}

fn shutdown(noexec: bool, mut writer: WriterWithTCP) {
    println!("Interrupt signal. Shutting down...");
    let result = unsafe {MANAGER.as_ref().unwrap().shutdown(noexec, &mut writer)};
    if let Err(e) = result {
        println!("{}", e.to_string());
    }
    exit(1);
}