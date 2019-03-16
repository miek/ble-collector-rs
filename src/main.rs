extern crate ble_advert_struct;
extern crate rumble;
extern crate rumqtt;
extern crate serde_json;
 
use std::collections::HashMap;
use std::env;
use std::thread;
use std::time::{Duration, SystemTime};

use ble_advert_struct::BLEAdvert;
use rumble::bluez::manager::Manager;
use rumble::api::{Central, Peripheral};
use rumqtt::{MqttClient, MqttOptions, QoS};

pub fn main() {
    let mqtt_host = env::var("MQTT_HOST").unwrap();
    let mqtt_topic = env::var("MQTT_TOPIC").unwrap();
    let mqtt_options = MqttOptions::new("ble-collector", mqtt_host, 1883);
    let (mut mqtt_client, _notifications) = MqttClient::start(mqtt_options).unwrap();

    let manager = Manager::new().unwrap();
 
    // get the first bluetooth adapter
    let adapters = manager.adapters().unwrap();
    let mut adapter = adapters.into_iter().nth(0).unwrap();
 
    // reset the adapter -- clears out any errant state
    adapter = manager.down(&adapter).unwrap();
    adapter = manager.up(&adapter).unwrap();
 
    // connect to the adapter
    let central = adapter.connect().unwrap();

    // Passive scan
    central.active(false);
    // Don't filter duplicates
    central.filter_duplicates(false);
 
    // start scanning for devices
    central.start_scan().unwrap();

    let mut last_seen = HashMap::new();
 
    loop {
        for p in central.peripherals() {
            let prop = p.properties();
            let ls = last_seen.entry(prop.address).or_insert(0);
            if *ls != prop.discovery_count {
                if let Some(data) = prop.manufacturer_data {
                    let advert = BLEAdvert {
                        manufacturer_data: data,
                        mac: prop.address.to_string(),
                        time: SystemTime::now(),
                        // TODO: get local hostname
                        listener: "changeme".to_string(),
                    };
                    let json = serde_json::to_string(&advert).unwrap();
                    mqtt_client.publish(mqtt_topic.clone(), QoS::AtLeastOnce, false, json).unwrap();
                }
            }
            *ls = prop.discovery_count;
        }
        thread::sleep(Duration::from_millis(1000));
    }
}
