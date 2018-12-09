extern crate byteorder;
#[macro_use] extern crate mac;
extern crate rumble;
 
use std::collections::HashMap;
use std::io::Cursor;
use std::thread;
use std::time::Duration;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use rumble::bluez::manager::Manager;
use rumble::api::{Central, Peripheral};

#[derive(Debug)]
struct RuuviPacket {
    version: u32,
    humidity: f32,
    temperature: f32,
    pressure: f32,
    acceleration_x: f32,
    acceleration_y: f32,
    acceleration_z: f32,
    voltage: f32,
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

    let humidity = humidity as f32 / 2.0;
    let sign = match temp_int & 0x80 == 0 {
        true => 1.0,
        false => -1.0,
    };
    let temperature = (temp_int & !(0x80)) as f32 * sign + temp_hundredths as f32 / 100.0;
    let pressure = pressure as f32 + 50000.0;
    let voltage = battery_mv as f32 / 1000.0;

    Some(RuuviPacket{
        version: 3,
        humidity: humidity,
        temperature: temperature,
        pressure: pressure,
        acceleration_x: accel_x as f32,
        acceleration_y: accel_y as f32,
        acceleration_z: accel_z as f32,
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
                if let Some(p) = ruuvi_packet {
                    println!("{:?}", p);
                }
            }
            *ls = prop.discovery_count;
        }
        thread::sleep(Duration::from_millis(1000));
    }
}
