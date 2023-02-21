use crate::{actor, net, scd30, SensorMetrics};
use anyhow::Context;
use embedded_svc::{
    http::{
        server::{Connection, HandlerResult, Request, Response},
        Method,
    },
    io::{Read, Write},
};
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use serde::Serialize;

pub struct Server {
    _server: EspHttpServer,
}

pub const HTTP_PORT: u16 = 80;
pub const HTTPS_PORT: u16 = 443;

pub fn start_server(
    wifi: &net::EclssWifi,
    metrics: &'static SensorMetrics,
    scd30_ctrl: actor::Client<scd30::ControlMessage, anyhow::Result<()>>,
) -> anyhow::Result<Server> {
    let mut server = EspHttpServer::new(&Configuration {
        http_port: HTTP_PORT,
        https_port: HTTPS_PORT,
        ..Default::default()
    })
    .context("failed to start HTTP server")?;
    let access_points = wifi.access_points.clone();
    let creds_tx = wifi.credentials_tx();
    server
        .fn_handler("/", Method::Get, move |req| {
            static INDEX: &[u8] = include_bytes!("./http/index.html");
            rsp_ok(req, content_type::HTML)?
            .write_all(INDEX)?;
            Ok(())
        })
        .context("adding GET / handler")?
        // TODO(eliza): also serve this on the normal prometheus metrics port?
        .fn_handler("/metrics", Method::Get, move |req| {
            let mut rsp = rsp_ok(req, "text/plain; version=0.0.4")?;
            metrics.render_prometheus(&mut rsp)?;
            Ok(())
        })
        .context("adding GET /metrics handler")?
        .fn_handler("/sensors.json", Method::Get, move |req| {
            serve_json(req, metrics)
        })
        .context("adding GET /sensors.json handler")?
        .fn_handler("/sensors/co2/calibrate", Method::Post, move |mut req| {
            // TODO(eliza): this needs to be authed...
            
            #[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
            struct Calibrate {
                ppm: u16,
            }

            let mut body = vec![0; 40];
            read_body(&mut req, &mut body)?;

            let Calibrate { ppm } = serde_urlencoded::from_bytes(&body)?;

            match scd30_ctrl
                .try_send(scd30::ControlMessage::ForceCalibrate { ppm })
            {
                Ok(_) => {
                    log::info!("sent request to calibrate {ppm} ppm");
                    let content = r#"<!DOCTYPE html><html><body>Submitted!</body></html>"#;
                    rsp_ok(req, content_type::HTML)?
                    .write_all(content.as_bytes())?;
                }
                Err(_) => {
                    log::warn!("calibration control channel error");
                    let content = format!(r#"<!DOCTYPE html><html><body>calibration control channel full</body></html>"#);
                    req.into_response(
                        500,
                        Some("Internal Server Error"),
                        &[(header::CONTENT_TYPE, content_type::HTML)],
                    )?
                    .write_all(content.as_bytes())?;
                }
            }

            Ok(())

        })
        .context("adding POST /sensors/co2/calibrate handler")?
        .fn_handler("/wifi/ssids.json", Method::Get, move |req| {
            let ssids = access_points.read().unwrap();
            let ssids = ssids.iter().map(|ap| &ap.ssid).collect::<Vec<_>>();
            serve_json(req, &ssids)
        })
        .context("adding GET /wifi/ssids.json handler")?
        .fn_handler("/wifi/select", Method::Post, move |mut req| {
            let mut body = vec![0; 40];
            read_body(&mut req, &mut body)?;

            let credentials = serde_urlencoded::from_bytes(&body)?;

            match creds_tx
                .try_send(credentials)
            {
                Ok(()) => {
                    log::info!("sent request to connect to WiFi network");
                    let content = r#"<!DOCTYPE html><html><body>Submitted!</body></html>"#;
                    rsp_ok(req, content_type::HTML)?
                    .write_all(content.as_bytes())?;
                }
                Err(error) => {
                    log::warn!("wifi control channel error: {error:?}");
                    let content = format!(r#"<!DOCTYPE html><html><body>Failed to select WiFI network: {error:?}</body></html>"#);
                    req.into_response(
                        500,
                        Some("Internal Server Error"),
                        &[(header::CONTENT_TYPE, content_type::HTML)],
                    )?
                    .write_all(content.as_bytes())?;
                }
            }

            Ok(())
        })
        .context("adding POST /wifi/select handler")?;

    log::info!("Server is running on http://192.168.71.1/");

    Ok(Server { _server: server })
}

fn serve_json<C: Connection>(req: Request<C>, json: &impl Serialize) -> HandlerResult {
    // XXX(eliza): this is technically more correct but i wanna be able to open
    // it in the browser...
    /*
    if let Some(accept) = req.header("accept") {
        if !accept.contains(JSON) {
            req.into_status_response(406)? // not acceptable
                .write_all(JSON.as_bytes())?;
            return Ok(());
        }
    }
    */

    let mut rsp = rsp_ok(req, content_type::JSON)?;
    // TODO(eliza): don't allocate here...
    let json = serde_json::to_string_pretty(&json)?;
    rsp.write_all(json.as_bytes())?;
    Ok(())
}

fn read_body<R: Read>(response: &mut R, buf: &mut Vec<u8>) -> anyhow::Result<usize> {
    let mut total_bytes_read = 0;

    while let Ok(bytes_read) = response.read(&mut buf[total_bytes_read..]) {
        log::trace!("read {bytes_read} bytes");
        if bytes_read == 0 {
            break;
        } else {
            total_bytes_read += bytes_read;
            buf.resize(buf.len() * 2, 0);
        }
    }

    anyhow::ensure!(total_bytes_read > 0, "empty request body");

    buf.truncate(total_bytes_read);

    Ok(total_bytes_read)
}

fn rsp_ok<C: Connection>(
    req: Request<C>,
    content_type: &'static str,
) -> Result<Response<C>, C::Error> {
    req.into_response(200, Some("OK"), &[(header::CONTENT_TYPE, content_type)])
}

mod header {
    pub(super) const CONTENT_TYPE: &str = "content-type";
}

mod content_type {

    pub(super) const JSON: &str = "application/json";
    pub(super) const HTML: &str = "text/html";
}
