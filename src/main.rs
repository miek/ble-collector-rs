extern crate rumble;
 
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rumble::bluez::manager::Manager;
use rumble::api::{Central, Peripheral};

pub fn main() {
    let manager = Manager::new().unwrap();
 
    // get the first bluetooth adapter
    let adapters = manager.adapters().unwrap();
    let mut adapter = adapters.into_iter().nth(0).unwrap();
 
    // reset the adapter -- clears out any errant state
    adapter = manager.down(&adapter).unwrap();
    adapter = manager.up(&adapter).unwrap();
 
    // connect to the adapter
    let central = adapter.connect().unwrap();
 
    // start scanning for devices
    central.start_scan().unwrap();

    let mut last_seen = HashMap::new();
 
    loop {
        for p in central.peripherals() {
            let prop = p.properties();
            let ls = last_seen.entry(prop.address).or_insert(0);
            if *ls != prop.discovery_count {
                if let Some(_d) = prop.manufacturer_data {
                    // TODO: punt to mqtt
                }
            }
            *ls = prop.discovery_count;
        }
        thread::sleep(Duration::from_millis(1000));
    }
}
