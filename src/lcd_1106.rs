use esp_idf_hal::{i2c::{I2cDriver, I2cConfig}};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use sh1106::{prelude::*, Builder};
use anyhow::{Result};

use esp_idf_hal::peripheral;
use esp_idf_hal::gpio;
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;

pub fn lcd_display_1106(
    display: &mut GraphicsMode<I2cInterface<I2cDriver<'static>>>,
    line1: &str,
    line2: &str,
) -> Result<()> {
    display.clear();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline(line1, Point::zero(), text_style, Baseline::Top)
        .draw(display)
        .unwrap();

    Text::with_baseline(line2, Point::new(0, 16), text_style, Baseline::Top)
        .draw(display)
        .unwrap();

    display.flush().unwrap();

    Ok(())
}

pub fn init_display_1106(
    i2c0: impl peripheral::Peripheral<P = impl i2c::I2c> + 'static, // peripherals.i2c0, // i2c
    scl: gpio::AnyIOPin, // pins.gpio22.into(), // scl
    sda: gpio::AnyIOPin, // pins.gpio21.into(), // sda
) -> Result<GraphicsMode<I2cInterface<I2cDriver<'static>>>> {
    let config = I2cConfig::new().baudrate(400.kHz().into());
    let mut i2c = I2cDriver::new(i2c0, sda, scl, &config)?;

    let mut display1: GraphicsMode<_> = Builder::new().connect_i2c(i2c
        // sh1106::displaysize::DisplaySize::DisplaySize128x64,
        // sh1106::rotation::DisplayRotation::Rotate0,
    ).into();

    display1.init().unwrap();
    display1.flush().unwrap();

    Ok(display1)
}