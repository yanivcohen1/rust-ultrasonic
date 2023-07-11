//! HTTP/WebSocket Server with contexts
//!
//! Go to http://192.168.1.47/ to play

use anyhow;
use embedded_svc::ws::FrameType;
use core::cmp::Ordering;

use esp_idf_hal::{prelude::Peripherals, modem::Modem, gpio::{PinDriver, Gpio34, Output}, 
    adc::{AdcChannelDriver, Atten11dB, AdcDriver, config::Config}};

use log::*;

use std::{borrow::Cow, collections::BTreeMap, str, sync::{Mutex, mpsc::SyncSender}};

use anyhow::{Result};

// yaniv add
// use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::peripheral;
// use esp_idf_hal::adc;
// use esp_idf_hal::delay;
use esp_idf_hal::gpio;
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;
// use esp_idf_hal::spi;

use esp_idf_hal::delay::FreeRtos;
use std::sync::mpsc;

mod ultrasonic;
use ultrasonic::calc_distance_cm;

mod lcd_1106;
use lcd_1106::{init_display_1106, lcd_display_1106};

mod statics;
use statics::{LED, DISPLAY, IP, SET_LEVEL, WS};

mod web_socket;
use web_socket::{main_ws};

// sliderPot = ADC(Pin(34))
// sliderPot.atten(ADC.ATTN_11DB) # Full range: 3.3v
// last_sliderPot = int(sliderPot.read() * 100 / 4095)

fn main() -> anyhow::Result<()> {

    let (tx, rx) = mpsc::sync_channel::<(String, String)>(100);
    #[allow(unused)]
    let peripherals = Peripherals::take().unwrap();
    #[allow(unused)]
    let pins = peripherals.pins;

    let mut adc1 = AdcDriver::new(peripherals.adc1, &Config::new().calibration(true))?;
    let mut adc_pin: esp_idf_hal::adc::AdcChannelDriver<'_, Gpio34, Atten11dB<_>> =
        AdcChannelDriver::new(pins.gpio34)?;
    // adc1.read(&mut adc_pin).unwrap()* 100 / 4095 // for read it
    let mut trig = PinDriver::output(pins.gpio2)?;
    let echo = PinDriver::input(pins.gpio15)?;

    let mut led = PinDriver::output(pins.gpio0)?;
    let mut display = init_display_1106(
        peripherals.i2c0, pins.gpio22.into(), pins.gpio21.into()).unwrap();
        
    critical_section::with(|cs| LED.borrow_ref_mut(cs).replace(led));
    critical_section::with(|cs| DISPLAY.borrow_ref_mut(cs).replace(display));

    critical_section::with(|cs| {
        lcd_display_1106(
            &mut DISPLAY.borrow_ref_mut(cs).as_mut().unwrap(),
            "Wait wifi connect..",
            ""
        );
    });
    
    main_ws(peripherals.modem, tx);
    println!("connected start user game");

    let mut load =  false;
    loop{
        if &*IP.lock().unwrap() != "" { // !=
            if !load {
                critical_section::with(|cs| {
                    lcd_display_1106(
                        &mut DISPLAY.borrow_ref_mut(cs).as_mut().unwrap(),
                        ("IP: ".to_owned() + &*IP.lock().unwrap()).as_str(),
                        ""
                    );
                });
                load = true;
            } 
            let distance_leve = calc_distance_cm(&mut trig, &echo).unwrap();
            let adc_level = adc1.read(&mut adc_pin).unwrap();// as i32 * (100 / 4095)
            let adc_level_i16 =  i32::from(adc_level) * 100 / 4095;
            // FreeRtos::delay_ms(10000);
            critical_section::with(|cs| {
                println!("distance: {},  adc: {}", &distance_leve, &adc_level_i16);
                unsafe { 
                    if !WS.borrow_ref_mut(cs).as_mut().is_none() {
                        (*WS.borrow_ref_mut(cs).as_mut().unwrap().ptr).send(FrameType::Text(false), 
                            format!("dis: {}, adc: {}", &distance_leve, &adc_level_i16).as_ref());
                            lcd_display_1106(
                                &mut DISPLAY.borrow_ref_mut(cs).as_mut().unwrap(),
                                format!("dis: {}, adc: {}", &distance_leve, &adc_level_i16).as_str(),
                                ""
                            );
                    }
                }
            });
        }
        FreeRtos::delay_ms(1000);
    }
    Ok(())
}
