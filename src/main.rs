#![feature(plugin)]
#![plugin(docopt_macros)]
#![feature(ip_addr)]
#![feature(test)]
#![feature(duration)]
#![feature(socket_timeout)]
#![allow(dead_code)]
#![feature(path_ext)]

extern crate rustc_serialize;
extern crate docopt;
extern crate toml;
extern crate msgpack;
extern crate byteorder;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate rand;
extern crate time;

use std::net::TcpListener;
use std::thread;
use std::io::prelude::*;
use std::fs::File;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};

use docopt::Docopt;

use toml::Value;

use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, Mutex};

#[macro_use] mod util;
mod encd;
mod internode;
mod node;
mod crc32;
mod db;
mod dbiface;
mod dbclient;

use dbiface::DbIface;
use internode::MeState;
use internode::InternodeService;


docopt!(Args derive Debug, "
Zufar

Usage:
  zufar serve <host> <port>
  zufar serve <configfile>
  zufar --version

Options:
  -h --help             Show this screen.
  --version             Show version.
", arg_port: Option<i32>);


fn main() {

    env_logger::init().unwrap();

    let args: Args = Args::docopt().decode().unwrap_or_else(|e| e.exit());
    println!("{:?}", args);

    if (&args).flag_version{
        println!("version 0.1.0");
        return;
    }

    let mut api_address = format!("{}:{}", args.arg_host, args.arg_port.unwrap_or(9123));
    let mut node_address:String = String::new();
    let mut seeds:Vec<String> = Vec::new();

    if (&args.arg_configfile).len() > 0 {
        match File::open(&args.arg_configfile) {
            Ok(mut f) => {
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);

                //println!("cfg content: {}", s);

                let cfg = toml::Parser::new(&*s).parse().unwrap();
                println!("cfg: {:?}", cfg);

                match cfg.get("zufar"){
                    Some(&Value::Table(ref section)) => {
                        match section.get("listen_address"){
                            Some(&Value::String(ref hnp)) => api_address = hnp.clone(),
                            _ => err!(2, "No `listen_address` in configuration.")
                        }
                        match section.get("node_address"){
                            Some(&Value::String(ref _node_address)) => {
                                println!("node address: {}", _node_address);
                                node_address = _node_address.clone();
                            },
                            _ => err!(5, "No `node_address` in configuration.")
                        }
                        match section.get("seeds"){
                            Some(&Value::Array(ref _seeds)) => {
                                println!("seeds: ");
                                for seed in _seeds {
                                    println!(" + {}", seed);
                                    match seed {
                                        &Value::String(ref seed) => seeds.push(seed.clone()),
                                        _ => ()
                                    }
                                    
                                }
                            },
                            _ => err!(5, "No `seeds` in configuration.")
                        }

                    },
                    _ => err!(3, "No [zufar] section")
                }

            },
            Err(e) => {
                err!(4, "error: {}", e);
            }
        }
    }

    let (tx, rx) = channel();
    //if node_address.len() > 0 {
        let mut inode = InternodeService::new(&node_address, &api_address, seeds, tx);
        
        InternodeService::setup_internode_communicator(&mut inode.clone());
    //}
    
    let db_iface = DbIface::new(inode.clone(), rx);
    
    db_iface.start();

    if args.cmd_serve {
        serve(&api_address, db_iface);
    }
}




fn serve(api_address:&String, db_iface:DbIface){
    
    let db_iface = Arc::new(Mutex::new(db_iface));
    
    let listener = TcpListener::bind(&**api_address).unwrap();
    println!("client comm listening at {} ...", api_address);
    for stream in listener.incoming() {
        
        let db_iface = db_iface.clone();
        
        thread::spawn(move || {
            let mut stream = stream.unwrap();
            stream.write(b"Welcome to Zufar\r\n").unwrap();
            
            'the_loop: loop {
                let mut buff = vec![0u8; 100];
                match stream.read(&mut buff){
                    Ok(count) if count > 0 => {
                        let data = &buff[0..count];
                        let mut db_iface = db_iface.lock().unwrap();
                        match db_iface.handle_packet(&mut stream, data){
                            Ok(i) if i > 0 =>
                                break 'the_loop
                            ,
                            _ => ()
                        }
                    },
                    Err(e) => panic!("error when reading. {}", e),
                    _ => ()
                }
            }
            
        });
    }
}





