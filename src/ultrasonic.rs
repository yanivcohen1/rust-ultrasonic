use std::ops::Sub;

use core::{borrow::BorrowMut, cell::RefCell};
use critical_section::Mutex as CS_Mutex;

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{*, self};
use esp_idf_hal::peripherals::Peripherals;

use esp_idf_svc::systime::EspSystemTime;

pub fn calc_distance_cm(trig: &mut PinDriver<'_, Gpio2, Output>, echo: &PinDriver<'_, Gpio15, Input>) -> anyhow::Result<(u128)> {
    // esp_idf_sys::link_patches();

    // Application Loop
    // loop {
        let mut vec: Vec<u128> = Vec::new();
        for n in 1..11 {
            // 1) Set pin ouput to low for 5 us to get clean low pulse
            trig.set_low()?;
            FreeRtos::delay_us(5_u32);

            // 2) Set pin output to high (trigger) for 10us
            trig.set_high()?;
            FreeRtos::delay_us(10_u32);
            trig.set_low()?;

            // Wait until pin goes high
            while !echo.is_high() {}

            // Kick off timer measurement
            let echo_start = EspSystemTime {}.now();

            // Wait until pin goes low
            while !echo.is_low() {}

            // Collect current timer count
            let echo_end = EspSystemTime {}.now();

            // Calculate the elapsed timer count
            let echo_dur = echo_end.sub(echo_start);

            // Calculate the distance in cms using formula in datasheet
            let distance_cm = echo_dur.as_micros();

            vec.push(distance_cm);
        }

        vec.sort();
        let distance_cm = vec[5] * 10 / 16 / 58;
        // Print the distance output
        println!("Distance {} cm\r", distance_cm);

        FreeRtos::delay_ms(500);

        Ok(distance_cm)
    // }
}