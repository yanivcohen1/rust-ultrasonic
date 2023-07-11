use esp_idf_hal::{modem::Modem};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    systime::EspSystemTime,
    wifi::{BlockingWifi, EspWifi},
    http::server::EspHttpServer
};
use esp_idf_svc::http::server::ws::EspHttpWsConnection;

use embedded_svc::{
    http::Method,
    wifi::{self, AccessPointConfiguration, AuthMethod, Configuration, ClientConfiguration},
    ws::FrameType,
    ipv4,
};

use std::{borrow::{Cow, BorrowMut}, collections::BTreeMap, str, sync::{Mutex, mpsc::SyncSender}};

use log::*;
use core::cmp::Ordering;

use esp_idf_sys::{self as _, EspError, ESP_ERR_INVALID_SIZE};

use crate::lcd_1106;
use lcd_1106::lcd_display_1106;

use crate::statics;
use statics::{LED, DISPLAY, IP, WS};

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");
static INDEX_HTML: &str = include_str!("www/ws_guessing_game.html");

// Max payload length
const MAX_LEN: usize = 8;

// Need lots of stack to parse JSON
const STACK_SIZE: usize = 10240;

// Wi-Fi channel, between 1 and 11
const CHANNEL: u8 = 11;

struct GuessingGame {
    guesses: u32,
    secret: u32,
    done: bool,
}


pub fn main_ws(modem: Modem , tx: SyncSender<(String, String)>) -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // main_lib_fun();
    let mut server= create_server_new(modem)?;

    server.fn_handler("/", Method::Get, |req| {
        let headers = [("content-type", "text/html")];
        req.into_response(200, Some(&""), &headers)?
                    //.write(include_str!("www/index.html").as_bytes())?;
                    .write(INDEX_HTML.replace("%%IP%%", &*IP.lock().unwrap()).as_bytes())?;
        Ok(())
    })?
    .fn_handler("/favicon.ico", Method::Get, |req| {
        let headers = [("content-type", "image/x-icon")];
        req.into_response(200, Some(&""), &headers)?
                    .write(include_bytes!("www/favicon.ico"))?;
        Ok(())
    })?
    .fn_handler("/chart.min.js", Method::Get, |req| {
        let headers = [("content-type", "application/javascript")];
        req.into_response(200, Some(&""), &headers)?
                    .write(include_str!("www/chart.min.js").as_bytes())?;
        Ok(())
    })?
    .fn_handler("/style.css", Method::Get, |req| {
        let headers = [("content-type", "text/css")];
        req.into_response(200, Some(&""), &headers)?
                    .write(include_str!("www/style.css").as_bytes())?;
        Ok(())
    })?
    .fn_handler("/ultrasonic_page.html", Method::Get, |req| {
        let headers = [("content-type", "text/html")];
        req.into_response(200, Some(&""), &headers)?
                    .write(include_str!("www/ultrasonic_page.html").as_bytes())?;
        Ok(())
    })?;

    let guessing_games = Mutex::new(BTreeMap::<i32, GuessingGame>::new());

    server
        .ws_handler("/ws/guess", move |ws| {
            let mut sessions = guessing_games.lock().unwrap();
            if ws.is_new() {
                let num_to_guess = (rand() % 100) + 1;
                sessions.insert(ws.session(), GuessingGame::new(num_to_guess));
                info!("New WebSocket session ({} open)", sessions.len());
                println!("num_to_guess: {}", &num_to_guess);
                ws.send(
                    FrameType::Text(false),
                    "Welcome to the guessing game! Enter a number between 1 and 100".as_bytes(),
                )?;
                critical_section::with(|cs| WS.borrow_ref_mut(cs).replace(
                    statics::NoSendStruct { ptr: ws }));
                return Ok(());
            } else if ws.is_closed() {
                sessions.remove(&ws.session());
                info!("Closed WebSocket session ({} open)", sessions.len());
                return Ok(());
            }
            let session = sessions.get_mut(&ws.session()).unwrap();

            // NOTE: Due to the way the underlying C implementation works, ws.recv()
            // may only be called with an empty buffer exactly once to receive the
            // incoming buffer size, then must be called exactly once to receive the
            // actual payload.
            //critical_section::with(|cs| WS.borrow_ref_mut(cs).replace(
                //statics::NoSendStruct { ptr: ws }));
    
                let (_frame_type, len) = match ws.recv(&mut []) {
                    Ok(frame) => frame,
                    Err(e) => return Err(e),
                }; 
                
                if len > MAX_LEN {
                    ws.send(FrameType::Text(false), "Request too big".as_bytes())?;
                    ws.send(FrameType::Close, &[])?;
                    return Err(EspError::from_infallible::<ESP_ERR_INVALID_SIZE>());
                }
    
                let mut buf = [0; MAX_LEN]; // Small digit buffer can go on the stack
                ws.recv(buf.as_mut())?;
                let Ok(user_string) = str::from_utf8(&buf[..len]) else {
                    ws.send(FrameType::Text(false), "[UTF-8 Error]".as_bytes())?;
                    return Ok(());
                };

            

                let Some(user_guess) = GuessingGame::parse_guess(user_string) else {
                    ws.send(FrameType::Text(false), "Please enter a number between 1 and 100".as_bytes())?;
                    return Ok(());
                };

                let mut msg = "".to_owned();
                match session.guess(user_guess) {
                    (Ordering::Greater, n) => {
                        let reply = format!("Your {} guess was too high", nth(n));
                        ws.send(FrameType::Text(false), reply.as_ref())?;
                        *&mut msg = "guess too high".to_owned();
                    }
                    (Ordering::Less, n) => {
                        let reply = format!("Your {} guess was too low", nth(n));
                        ws.send(FrameType::Text(false), reply.as_ref())?;
                        *&mut msg = "guess too low".to_owned();
                    }
                    (Ordering::Equal, n) => {
                        let reply = format!(
                            "You guessed {} on your {} try! Refresh to play again",
                            session.secret,
                            nth(n)
                        );
                        ws.send(FrameType::Text(false), reply.as_ref())?;
                        ws.send(FrameType::Close, &[])?;
                        *&mut msg = "guess is correct!!".to_owned();
                    }
                }
                
            critical_section::with(|cs| {
                let mut temp = LED.borrow_ref_mut(cs);
                let mut led = temp.as_mut().unwrap();
                if msg.as_str().contains("correct") {
                    led.set_high();
                } else {
                    led.set_low();
                }
                
                lcd_display_1106(
                    &mut DISPLAY.borrow_ref_mut(cs).as_mut().unwrap(),
                    format!("user in: {}", user_guess).as_str(),
                    format!("{}", msg).as_str()
                );
                // ws Keep server running beyond
                core::mem::forget(ws);
            });
            // tx.send((format!("user in: {}", user_guess), msg)).unwrap();
            Ok::<(), EspError>(())
        })
        .unwrap();

    // Keep server running beyond when main() returns (forever)
    // Do not call this if you ever want to stop or access it later.
    // Otherwise you can either add an infinite loop so the main task
    // never returns, or you can move it to another thread.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    core::mem::forget(server);

    // Main task no longer needed, free up some memory
    Ok(())
}


#[allow(dead_code)]
fn create_server_new(modem: Modem) -> anyhow::Result<EspHttpServer> {
    // let peripherals = Peripherals::take().unwrap();
    // let modem = peripherals.modem;
    let sysloop1 = EspSystemEventLoop::take()?;
    let sysloop: EspSystemEventLoop = sysloop1.clone();

    use std::net::Ipv4Addr;

    use esp_idf_svc::handle::RawHandle;

    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

    info!("Starting wifi...");

    wifi.start()?;

    info!("Scanning...");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: SSID.into(),
            password: PASSWORD.into(),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    info!("Connecting wifi...");

    wifi.connect()?;

    info!("Waiting for DHCP lease...");

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    let ip = ip_info.ip; // .unwrap()

    println!("Wifi IP: {}", &ip);
    *IP.lock().unwrap() = String::from(&ip.to_string()); //ip.to_string();
    // ping(ip_info.subnet.gateway)?;

    // Ok(Box::new(esp_wifi))

    // yaniv
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    // Keep wifi running beyond when this function returns (forever)
    // Do not call this if you ever want to stop or access it later.
    // Otherwise it should be returned from this function and kept somewhere
    // so it does not go out of scope.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    core::mem::forget(wifi);
    core::mem::forget(esp_wifi);

    Ok(EspHttpServer::new(&server_configuration)?)
}



impl GuessingGame {
    fn new(secret: u32) -> Self {
        Self {
            guesses: 0,
            secret,
            done: false,
        }
    }

    fn guess(&mut self, guess: u32) -> (Ordering, u32) {
        if self.done {
            (Ordering::Equal, self.guesses)
        } else {
            self.guesses += 1;
            let cmp = guess.cmp(&self.secret);
            if cmp == Ordering::Equal {
                self.done = true;
            }
            (cmp, self.guesses)
        }
    }

    fn parse_guess(input: &str) -> Option<u32> {
        // Trim control codes (including null bytes) and/or whitespace
        let Ok(number) = u32::from_str_radix(input.trim_matches(|c: char| {
            c.is_ascii_control() || c.is_whitespace()
        }), 10) else {
            warn!("Not a number: `{}` (length {})", input, input.len());
            return None;
        };
        if !(1..=100).contains(&number) {
            warn!("Not in range ({})", number);
            return None;
        }
        Some(number)
    }
}

// Super rudimentary pseudo-random numbers
fn rand() -> u32 {
    EspSystemTime::now(&EspSystemTime {}).subsec_nanos() / 65537
}

// Serialize numbers in English
fn nth(n: u32) -> Cow<'static, str> {
    match n {
        smaller @ (0..=13) => Cow::Borrowed(match smaller {
            0 => "zeroth",
            1 => "first",
            2 => "second",
            3 => "third",
            4 => "fourth",
            5 => "fifth",
            6 => "sixth",
            7 => "seventh",
            8 => "eighth",
            9 => "ninth",
            10 => "10th",
            11 => "11th",
            12 => "12th",
            13 => "13th",
            _ => unreachable!(),
        }),
        larger => Cow::Owned(match larger % 10 {
            1 => format!("{}st", larger),
            2 => format!("{}nd", larger),
            3 => format!("{}rd", larger),
            _ => format!("{}th", larger),
        }),
    }
}