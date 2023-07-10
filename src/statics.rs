
use esp_idf_hal::{prelude::Peripherals, modem::Modem, i2c::{I2cDriver, I2cConfig}, 
                    gpio::{PinDriver, Gpio0, Output}};

use core::{borrow::BorrowMut, cell::RefCell};
use critical_section::Mutex as CS_Mutex;
use std::{borrow::Cow, collections::BTreeMap, str, sync::{Mutex, mpsc::SyncSender}};
use sh1106::{prelude::*, Builder};
use esp_idf_svc::http::server::ws::EspHttpWsConnection;

pub struct NoSendStruct {
    pub ptr: *mut EspHttpWsConnection,
}

unsafe impl Send for NoSendStruct {}

pub static CS: esp_idf_hal::task::CriticalSection = esp_idf_hal::task::CriticalSection::new();
pub static LED: CS_Mutex<RefCell<Option<PinDriver<'_, Gpio0, Output>>>> = CS_Mutex::new(RefCell::new(None));
pub static DISPLAY: CS_Mutex<RefCell<Option<GraphicsMode<I2cInterface<I2cDriver<'static>>>>>> = CS_Mutex::new(RefCell::new(None));
pub static WS: CS_Mutex<RefCell<Option<NoSendStruct>>> = CS_Mutex::new(RefCell::new(None));
pub static IP: Mutex<String> = Mutex::new(String::new());
pub static SET_LEVEL: Mutex<i32> = Mutex::new(0);

pub struct GuessingGame {
    guesses: u32,
    secret: u32,
    done: bool,
}

pub struct SendData {
    distance: String,
    led: String,
    slider: String,
}

pub struct RecData {
    slider: String,
}