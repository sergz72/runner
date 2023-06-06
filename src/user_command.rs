use std::io::{Error, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use crate::service_manager::ServiceManager;

pub struct WriterWithTCP {
    stream: Option<TcpStream>
}

impl WriterWithTCP {
    pub fn new(stream: Option<TcpStream>) -> WriterWithTCP {
        WriterWithTCP{ stream }
    }

    pub fn write_string(&mut self, string: String) -> Result<(), Error> {
        println!("{}",  string);
        if let Some(w) = &mut self.stream {
            writeln!(w, "{}", string)?;
            w.flush()?;
        }
        Ok(())
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

pub fn run_user_command(command: String, manager: &'static ServiceManager, noexec: bool, writer: &mut WriterWithTCP)
    -> Result<(), Error> {
    writer.write_string(format!("Running command {}", command))?;
    if command.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "empty command"));
    }
    let parts: Vec<&str> = command.split(&[' ', '_']).collect();
    return match parts[0] {
        "up" => if parts.len() == 2 {
            manager.up(parts[1], noexec, writer)?;
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "down" => if parts.len() == 1 {
            manager.shutdown(noexec, writer)?;
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "start" => if parts.len() == 2 {
            if parts[1].contains('.') {
                manager.start_script(false, parts[1], noexec, writer)?;
            } else {
                manager.start_service(false, parts[1], noexec, writer)?;
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "force-start" => if parts.len() == 2 {
            if parts[1].contains('.') {
                manager.start_script(true, parts[1], noexec, writer)?;
            } else {
                manager.start_service(true, parts[1], noexec, writer)?;
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "stop" => if parts.len() == 2 {
            if parts[1].contains('.') {
                manager.stop_script(parts[1], writer)?;
            } else {
                manager.stop_service(parts[1], noexec, writer)?;
            }
            Ok(())
        } else { Err(build_invalid_command_error()) },
        "status" => if parts.len() == 1 {
            writer.write_string(manager.report_status())?;
            Ok(())
        } else { Err(build_invalid_command_error()) },
        _ => Err(Error::new(ErrorKind::InvalidInput, "unknown command"))
    };
}

fn build_invalid_command_error() -> Error {
    Error::new(ErrorKind::InvalidInput, "invalid command")
}

pub fn run_user_commands(commands: Vec<String>, manager: &'static ServiceManager, noexec: bool, mut writer: WriterWithTCP) {
    for command in commands {
        if let Err(e) = run_user_command(command, manager, noexec, &mut writer) {
            let _ = writer.write_string(format!("{}", e));
            return;
        }
    }
}