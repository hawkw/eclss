# Environmental Control and Life Support Systems (ECLSS)

a li'l IoT environmental sensor node that exposes data in [the prometheus
metrics format][prom].

## hardware

- **microcontroller**: ESP32-C3; [QT Py ESP32-C3 from Adafruit](https://www.adafruit.com/product/5405).
  could work with any ESP32-C3 board with a few source tweaks (which GPIO pins
  are I2C).
- **sensors**:
  + **Sensirion SCD30** NDIR CO<sub>2</sub> sensor (with temperature and relative
    humidity); [Adafruit breakout board](https://www.adafruit.com/product/4867).
  + **Bosch BME680** temperature, barometric pressure, humidity, and VOC (MOX gas
    sensor); [Adafruit breakout board](https://www.adafruit.com/product/3660). i
    meant to get the slightly newer BME688 breakout but i clicked the
    wrong one. BME688 would also work.
  + **Plantower PMSA003I** particulate sensor (PLANNED); [Adafruit breakout
    board](https://www.adafruit.com/product/4632). i haven't actually bought
    this one yet.
- **human interface**
  + none yet! i'm thinking an e-ink display might be cool...

## software

- runs a WiFi access point (SSID: `eclss`) for configuration. connect to `eclss`
  and open `http://192.168.71.1` to configure the SSID and password of a WiFi
  access point to connect to.
- exposes an HTTP server on port 80 with the configuration interface at `/` and
  [prometheus metrics][prom] at `/metrics`.
- advertises the following mDNS services:
  + `_http._tcp`
  + `_https._tcp`
  + `_prometheus-http._tcp`
  + `_prometheus-https._tcp`
- the `_prometheus-http`/`_prometheus-https` mDNS services would allow something
  like [`msiebuhr/prometheus-mdns-sd`] to automatically discover ECLSS scrape
  targets.

[prom]: https://prometheus.io/docs/instrumenting/exposition_formats/#text-based-format
[`msiebuhr/prometheus-mdns-sd`]: https://github.com/msiebuhr/prometheus-mdns-sd