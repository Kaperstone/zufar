

use std::time::Duration;
//use std;
use std::thread;
//use std::net::TcpListener;
use std::net::{TcpStream, SocketAddr};
use std::io::prelude::*;
//use std::str;
use std::cell::{RefCell, RefMut};
//use std::io::BufReader;
use std::error::Error;
use std::net::Shutdown;

//use time::Duration;


pub trait RetryPolicy {
    fn new() -> Self;
    fn should_retry(&mut self) -> bool;
    fn delay(&self) -> u32;
    fn tried(&self) -> u16;
    fn reset(&mut self);
}

#[derive(Debug)]
pub struct BackoffRetryPolicy {
    delay:u32,
    tried:u16
}


impl RetryPolicy for BackoffRetryPolicy {
    fn new() -> BackoffRetryPolicy {
        BackoffRetryPolicy {
            delay: 1000,
            tried: 0
        }
    }
    fn should_retry(&mut self) -> bool {
        self.tried = self.tried + 1;
        self.tried < 10
    }
    fn delay(&self) -> u32 {
        self.delay * (self.tried as u32)
    }
    fn tried(&self) -> u16 {
        self.tried
    }
    fn reset(&mut self){
        self.tried = 0;
    }
}

// #[derive(Debug)]
// enum RetryPolicyType {
//     Backoff
// }


type DbcResult = Result<String,&'static str>;

#[derive(Debug)]
pub struct DbClient<T> where T:RetryPolicy {
    address: String,
    stream: RefCell<Option<TcpStream>>,
    retry_policy: T
}

impl<T> DbClient<T> where T:RetryPolicy {
    
    pub fn new(address:&String, rp:T) -> DbClient<T> {
        DbClient {
            address: address.clone(),
            stream: RefCell::new(None),
            retry_policy: rp
        }
    }
    
    pub fn connect(&self) -> Result<u16, &'static str> {
        let addr:SocketAddr = self.address.parse().unwrap();
        match TcpStream::connect(addr){
            Ok(stream) => {

                let _ = stream.set_read_timeout(Some(Duration::new(5, 0)));
                
                // clean up welcome message
                //let _ = stream.read(&mut [0u8; 128]);
                // trace!("before borrow");
                let mut s = self.stream.borrow_mut();
                // trace!("after borrow, to write.");
                *s = Some(stream);
                // trace!("after write");
             
                Ok(0)
            },
            Err(e) => {
                error!("cannot connect to {}, {}", &self.address, e);
                Err("cannot connect")
            }
        }
    }
    
    fn reconnect<'a>(&'a mut self, s:&'a mut RefMut<'a, Option<TcpStream>>){
        let addr:SocketAddr = self.address.parse().unwrap();
        match TcpStream::connect(addr){
            Ok(stream) => {

                let _ = stream.set_read_timeout(Some(Duration::new(5, 0)));
                
                // clean up welcome message
                //let _ = stream.read(&mut [0u8; 128]);
                trace!("before borrow");
                //let mut s = self.stream.borrow_mut();
                trace!("after borrow, to write.");
                **s = Some(stream);
                trace!("after write");
             
                // Ok(0)
            },
            Err(e) => {
                error!("cannot connect to {}, {}", &self.address, e);
                // Err("cannot connect")
            }
        }
    }
    
    // fn get_retry_policy(&self) -> T {
    //     match self.retry_policy {
    //         RetryPolicyType::Backoff => {
    //             BackoffRetryPolicy::new()
    //         }
    //     }
    // }
    
    pub fn set(&mut self, key:&str, v:&str){
        let s = self.stream.borrow_mut();
        
        if s.is_some() {
            let mut stream = s.as_ref().unwrap();
            let data = format!("set {} 0 0 {} \r\n", key, v.len());
            let _ = stream.write(data.as_bytes());
            // let _ = stream.flush();
            
            // let _ = stream.read(&mut [0u8; 512]);
            
            let _ = stream.write(v.as_bytes());
            let _ = stream.flush();
            let _ = stream.read(&mut [0u8; 512]);
        }
    }
    
    pub fn get_raw(&mut self, key:&str) -> Result<String,&str> {
        // let s = self.stream.borrow_mut();
        
        // if s.is_some() {
            
            {
                let s = self.stream.borrow_mut();
                let mut stream = s.as_ref().unwrap();
                let data = format!("get {}", key);

                trace!("querying server with: {}", data);

                let _ = stream.write(data.as_bytes());
                let _ = stream.flush();
            }
            
            let mut buff = vec![0u8; 256];
            
            trace!("reading...");
            
            let result = 
            {
                let s = self.stream.borrow_mut();
                let mut stream = s.as_ref().unwrap();
                
                match stream.read(&mut buff) {
                    Ok(count) if count > 0 => {

                        trace!("done reading with {} bytes", count);

                        let content = String::from_utf8(buff[0..count].to_vec()).unwrap();

                        trace!("content: {}", content);

                        Ok(content)
                    },
                    Err(e) => {
                        error!("cannot read from stream. {}", e.description());
                        Err("")
                    },
                    x => { 
                        error!("unexpected return: {:?}", x);
                        Err("cannot read from remote node")
                    }
                }
            };
            
            if result.is_err() {
                if self.retry_policy.should_retry(){
                    warn!("retrying... ({})", self.retry_policy.tried());
                    self.connect();
                    
                    thread::sleep_ms(self.retry_policy.delay());
                }
                
                self.get_raw(key)
            }else{
                result
            }
            
        // }else{
        //     Err("cannot get stream")
        // }
    }
    
    // pub fn get_raw(&mut self, key:&str) -> Result<String,&str> {
    //     let mut done = false;
    //     let mut result:Result<String, &str> = Err("???");
    //     
    //     // let mut rp = &mut self.retry_policy;
    //     self.retry_policy.reset();
    //     
    //     
    //     while !done {
    //         //let raw_data = self.get_raw(key);
    //         result = self.get_raw_internal(key);
    //         
    //         trace!("result: {:?}", result);
    //         
    //         if result.is_ok(){
    //             done = true;
    //         }else{
    //             trace!("got error");
    //             if self.retry_policy.should_retry() {
    //                 warn!("reconnecting... ({})", self.retry_policy.tried());
    //                 self.connect();
    //                 thread::sleep_ms(self.retry_policy.delay());
    //                 //continue;
    //                 
    //             }else{
    //                 warn!("give up.");
    //                 done = true;
    //             }
    //         }
    //     }
    //     
    //     result
    // }
    
    pub fn get(&mut self, key:&str) -> Option<String> {
        
        // let mut done = false;
        // let mut result:Option<String> = None;
        // 
        // self.retry_policy.reset();
        // 
        // while !done {
        //     //let raw_data = self.get_raw(key);
        //     result = 
                match self.get_raw(key) {
                    // Ok() => {
                    //     warn!("return zero");
                    //     None
                    // },
                    Ok(d) => {
                        let s:Vec<&str> = d.split("\n").collect();
                        Some(s[1].to_string())
                    },
                    Err(e) => {
                        error!("error: {}", e);
                        None
                    }
                }
        //     
        //     trace!("result: {:?}", result);
        //     
        //     if result.is_some(){
        //         done = true;
        //     }else{
        //         trace!("got error");
        //         if self.retry_policy.should_retry() {
        //             warn!("reconnecting... ({})", self.retry_policy.tried());
        //             self.connect();
        //             thread::sleep_ms(self.retry_policy.delay());
        //             //continue;
        //             
        //         }else{
        //             warn!("give up.");
        //             done = true;
        //         }
        //     }
        // }
        // 
        // result
    }
    
    pub fn del(&mut self, key:&str) -> DbcResult {
        let stream = self.stream.borrow_mut();
        let mut stream = stream.as_ref().unwrap();
        let cmd = format!("del {}", key);
        let _ = stream.write(cmd.as_bytes());
        let mut buff = vec![0u8; 512];
        match stream.read(&mut buff){
            Ok(count) if count > 0 => {
                let rv = String::from_utf8(buff[0..count].to_vec()).unwrap();
                //if rv == "DELETED\r\n"
                Ok(rv)
            },
            Ok(_) => Err("count is zero"),
            Err(_) => Err("cannot read stream")
        }
    }
}



impl<T> Drop for DbClient<T> where T:RetryPolicy {
    fn drop(&mut self){
        debug!("db client shutdown.");
        self.stream.borrow_mut().as_ref()
            .map(|s| s.shutdown(Shutdown::Both));
    }
}


#[cfg(test)]
mod tests {
    
    use super::DbClient;
    
    fn get_db() -> DbClient {
        DbClient::new(&"127.0.0.1:8122".to_string())
    }
    
    #[test]
    fn test_set_n_get(){
        let mut dbc = get_db();
        let _ = dbc.connect();
        dbc.set("name", "Zufar");
        dbc.set("something", "In the way");
        dbc.set("article", "This is very long-long text we tried so far");
        assert_eq!(dbc.get_raw("name"), Ok("VALUE name 1 5\nZufar\nEND\n".to_string()));
        assert_eq!(dbc.get_raw("no_name"), Err("???"));
        assert_eq!(dbc.get("name"), Some("Zufar".to_string()));
        assert_eq!(dbc.get("none"), None);
        assert_eq!(dbc.get("something"), Some("In the way".to_string()));
        assert_eq!(dbc.get("article"), Some("This is very long-long text we tried so far".to_string()));
    }

}
