# ESP_IDF_VERSION=release/v4.4 
$Env:ESP_IDF_VERSION="release/v4.4"
$Env:WIFI_SSID="HOTBOX-89BA-yaniv"
$Env:WIFI_PASS=""
// flash
cargo espflash flash --release --monitor
cargo espflash flash --monitor

// build to image
cargo espflash save-image --chip esp32 build_image.bin
# cargo espflash --release --target xtensa-esp32-espidf --example ws_guessing_game --monitor

//flash ultrasonic
cargo espflash flash --example tests --monitor --release

// open the game
// http://192.168.71.1
http://192.168.1.47

// build
cargo espflash --release --monitor

// monitor
cargo espflash serial-monitor