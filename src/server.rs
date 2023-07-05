use std::io::{Error, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use crate::service_manager::ServiceManager;
use crate::user_command::{run_user_command, WriterWithTCP};

const PORT: u16 = 65000;

pub fn server_start(manager: &'static ServiceManager, noexec: bool) -> Result<(), Error> {
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], PORT)))?;
    println!("Server listening on port {}", PORT);
    for stream in listener.incoming() {
        match stream {
            Ok(s) => run_command(manager, noexec, WriterWithTCP::new(Some(s))),
            Err(e) => println!("Connection error {}", e.to_string())
        }
    }
    Ok(())
}

fn run_command(manager: &'static ServiceManager, noexec: bool, mut writer: WriterWithTCP) {
    let mut buffer = [0; 10000];
    match writer.read(&mut buffer) {
        Ok(amt) => {
            if amt == 0 {
                return;
            }
            if let Ok(command) = String::from_utf8(Vec::from(&buffer[0..amt])) {
                let parts = command.split(' ').map(|s|s.to_string()).collect();
                if let Err(e) = run_user_command(parts, manager, noexec, &mut writer) {
                    let _ = writer.write_string(format!("{}", e));
                }
            } else {
                println!("invalid command");
            }
        }
        Err(e) => println!("Stream read error {}", e.to_string())
    }
    println!("Stream shutdown");
    writer.shutdown();
}

pub fn send_command_to_server(command: String) -> Result<(), Error> {
    let mut buffer = [0; 10000];
    println!("Sending command {} to server...", command);
    let mut stream = TcpStream::connect(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), PORT))?;
    stream.write_all(command.as_bytes())?;
    loop {
        match stream.read(&mut buffer) {
            Ok(length) => {
                if length > 0 {
                    match String::from_utf8(Vec::from(&buffer[0..length])) {
                        Ok(s) => { print!("{}", s); continue; },
                        Err(_e) => println!("incorrect response from the server")
                    }
                }
                break;
            }
            Err(e) => {
                stream.shutdown(Shutdown::Both)?;
                return Err(e);
            }
        }
    }
    Ok(())
}