extern crate byteorder;
#[macro_use] extern crate influx_db_client;
#[macro_use] extern crate mac;
extern crate rumble;
 
use std::collections::HashMap;
use std::io::Cursor;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use influx_db_client::{Client, Point, Points, Value, Precision};
use rumble::bluez::manager::Manager;
use rumble::api::{Central, Peripheral};

#[derive(Debug)]
struct RuuviPacket {
    version: u32,
    mac: String,
    humidity: f64,
    temperature: f64,
    pressure: f64,
    acceleration_x: f64,
    acceleration_y: f64,
    acceleration_z: f64,
    voltage: f64,
}

fn decode_ruuvi_format_3(buf: &mut std::io::Read) -> Option<RuuviPacket> {
    if unwrap_or_return!(buf.read_u8().ok(), None) != 3 {
        return None;
    }
    let humidity = unwrap_or_return!(buf.read_u8().ok(), None);
    let temp_int = unwrap_or_return!(buf.read_u8().ok(), None);
    let temp_hundredths = unwrap_or_return!(buf.read_u8().ok(), None);
    let pressure = unwrap_or_return!(buf.read_u16::<BigEndian>().ok(), None);

    let accel_x = unwrap_or_return!(buf.read_i16::<BigEndian>().ok(), None);
    let accel_y = unwrap_or_return!(buf.read_i16::<BigEndian>().ok(), None);
    let accel_z = unwrap_or_return!(buf.read_i16::<BigEndian>().ok(), None);
    let battery_mv = unwrap_or_return!(buf.read_u16::<BigEndian>().ok(), None);

    let humidity = humidity as f64 / 2.0;
    let sign = match temp_int & 0x80 == 0 {
        true => 1.0,
        false => -1.0,
    };
    let temperature = (temp_int & !(0x80)) as f64 * sign + temp_hundredths as f64 / 100.0;
    let pressure = pressure as f64 + 50000.0;
    let voltage = battery_mv as f64 / 1000.0;

    Some(RuuviPacket{
        version: 3,
        mac: String::new(),
        humidity: humidity,
        temperature: temperature,
        pressure: pressure,
        acceleration_x: accel_x as f64,
        acceleration_y: accel_y as f64,
        acceleration_z: accel_z as f64,
        voltage: voltage,
    })
}

fn decode_ruuvi_packet(packet: &Vec<u8>) -> Option<RuuviPacket> {
    let mut buf = Cursor::new(packet);
    if buf.read_u16::<LittleEndian>().unwrap_or(0) != 0x0499 {
        return None;
    }
    [decode_ruuvi_format_3].iter().find_map(|f| f(&mut buf.clone()))
}

fn influx_post(pkt: &RuuviPacket) -> () {
    let client = Client::new("http://localhost:8086", "ruuvi");
    let mut point = point!("ruuvi_measurements");
    point.add_timestamp(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64);
    point.add_field("humidity", Value::Float(pkt.humidity));
    point.add_field("temperature", Value::Float(pkt.temperature));
    point.add_field("pressure", Value::Float(pkt.pressure));
    point.add_field("acceleration_x", Value::Float(pkt.acceleration_x));
    point.add_field("acceleration_y", Value::Float(pkt.acceleration_y));
    point.add_field("acceleration_z", Value::Float(pkt.acceleration_z));
    point.add_field("voltage", Value::Float(pkt.voltage));
    point.add_tag("mac", Value::String(pkt.mac.clone()));

    let points = points!(point);
    let _ = client.write_points(points, Some(Precision::Seconds), None).unwrap();
}
 
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
            let ruuvi_packet = match prop.manufacturer_data {
                Some(d) => decode_ruuvi_packet(&d),
                None => None,
            };
            let ls = last_seen.entry(prop.address).or_insert(0);
            if *ls != prop.discovery_count {
                if let Some(mut p) = ruuvi_packet {
                    p.mac = format!("{}", prop.address);
                    influx_post(&p);
                }
            }
            *ls = prop.discovery_count;
        }
        thread::sleep(Duration::from_millis(1000));
    }
}
