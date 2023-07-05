use std::collections::HashSet;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::process::exit;
use crate::service_manager::ServiceManager;

pub struct WriterWithTCP {
    stream: Option<TcpStream>
}

impl WriterWithTCP {
    pub fn new(stream: Option<TcpStream>) -> WriterWithTCP {
        WriterWithTCP{ stream }
    }

    pub fn write_string(&mut self, string: String) {
        println!("{}",  string);
        if let Some(w) = &mut self.stream {
            if writeln!(w, "{}", string).is_ok() {
                let _ = w.flush();
            }
        }
    }

    pub fn shutdown(&self) {
        if let Some(s) = &self.stream {
            if let Err(e) = s.shutdown(Shutdown::Both) {
                println!("Stream shutdown error {}", e);
            }
        }
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        self.stream.as_ref().unwrap().read(buffer)
    }
}

pub fn run_user_command(parts: Vec<String>, manager: &'static ServiceManager, noexec: bool, writer: &mut WriterWithTCP)
    -> Result<(), Error> {
    writer.write_string(format!("Running command {:?}", parts));
    if parts.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "empty command"));
    }
    return match parts[0].as_str() {
        "up" => if parts.len() == 2 {
            manager.up(&parts[1], noexec, writer)
        } else { Err(build_invalid_command_error()) },
        "down" => if parts.len() == 1 {
            manager.shutdown(noexec, writer)
        } else { Err(build_invalid_command_error()) },
        "start" => if parts.len() >= 2 {
            for i in 1..parts.len() {
                if parts[i].contains('.') {
                    manager.start_script(false, &parts[i], noexec, writer)?;
                } else {
                    manager.start_service(false, &parts[i], noexec, writer)?;
                }
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "force-start" => if parts.len() >= 2 {
            for i in 1..parts.len() {
                if parts[i].contains('.') {
                    manager.start_script(true, &parts[i], noexec, writer)?;
                } else {
                    manager.start_service(true, &parts[i], noexec, writer)?;
                }
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "stop" => if parts.len() >= 2 {
            for i in 1..parts.len() {
                if parts[i].contains('.') {
                    manager.stop_script(&parts[i], writer)?;
                } else {
                    manager.stop_service(&parts[i], noexec, writer)?;
                }
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "status" => if parts.len() == 1 {
            writer.write_string(manager.report_status(None));
            Ok(())
        } else {
            for i in 1..parts.len() {
                writer.write_string(manager.report_status(Some(&parts[i])));
            }
            Ok(())
        },
        "wait_for_scripts" => if parts.len() >= 2 {
            let scripts: HashSet<String> = parts.iter()
                .skip(1)
                .map(|s|s.clone())
                .collect::<HashSet<_>>();
            manager.wait_for_scripts(&scripts)
        } else { Err(build_invalid_command_error()) },
        "exit" => {
            let _ = manager.shutdown(noexec, writer);
            exit(0);
        },
        _ => Err(Error::new(ErrorKind::InvalidInput, "unknown command"))
    };
}

fn build_invalid_command_error() -> Error {
    Error::new(ErrorKind::InvalidInput, "invalid command")
}

pub fn run_user_commands(commands: Vec<String>, manager: &'static ServiceManager, noexec: bool, mut writer: WriterWithTCP) {
    if let Err(e) = run_user_command(commands, manager, noexec, &mut writer) {
        let _ = writer.write_string(format!("{}", e));
    }
}