use crate::{actor, net, sensor::{self, scd30}, SensorMetrics};
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
use std::fmt;

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
            rsp_ok(req, content_type::HTML)?.write_all(INDEX)?;
            Ok(())
        })
        .context("adding GET / handler")?
        // TODO(eliza): also serve this on the normal prometheus metrics port?
        .fn_handler("/metrics", Method::Get, move |req| {
            log::debug!("handling GET /metrics request...");
            let mut rsp = rsp_ok(req, "text/plain; version=0.0.4")?;
            write!(rsp, "{metrics}")?;
            log::debug!("metrics scrape OK!");
            Ok(())
        })
        .context("adding GET /metrics handler")?
        .fn_handler("/sensors.json", Method::Get, move |req| {
            log::debug!("handling GET /metrics request...");
            serve_json(req, metrics)
        })
        .context("adding GET /sensors.json handler")?
        .fn_handler("/sensors/status.json", Method::Get, move |req| {
            serve_json(req, &sensor::STATUSES)
        })
        .context("adding GET /sensors/status.json handler")?
        .fn_handler("/sensors/co2/calibrate", Method::Post, move |mut req| {
            // TODO(eliza): this needs to be authed...

            #[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
            struct Calibrate {
                ppm: u16,
            }

            let mut body = vec![0; 40];
            read_body(&mut req, &mut body)?;

            let ppm = match serde_urlencoded::from_bytes(&body) {
                Ok(Calibrate { ppm }) => ppm,
                Err(error) => return send_bad_request(req, error),
            };

            log::info!("received request to calibrate CO2 at {ppm} ppm");

            let send_fut = scd30_ctrl.try_request(scd30::ControlMessage::ForceCalibrate { ppm });
            // XXX(eliza): the use of `block_on` here is Unfortunate, switch to
            // an async HTTP server like `edge_net`...
            match futures::executor::block_on(send_fut) {
                Ok(_) => send_json_rsp(
                    req,
                    JsonResponse {
                        code: 200,
                        status: "OK",
                        message: "recalibrated SCD30",
                    },
                ),
                Err(_) => send_internal_error(req, "CO2 calibration channel error"),
            }
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

            let credentials = match serde_urlencoded::from_bytes(&body) {
                Ok(credentials) => credentials,
                Err(error) => return send_bad_request(req, error),
            };

            match creds_tx
                .try_send(credentials)
                .context("wifi control channel error")
            {
                Ok(_) => send_json_rsp(
                    req,
                    JsonResponse {
                        code: 200,
                        status: "OK",
                        message: "Connected",
                    },
                ),
                Err(error) => send_internal_error(req, error),
            }
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

    // TODO(eliza): don't allocate here
    match serde_json::to_string_pretty(&json) {
        Ok(json) => {
            log::debug!("responding with JSON: {json}");
            let mut rsp = rsp_ok(req, content_type::JSON)?;
            rsp.write_all(json.as_bytes())?;
        }
        Err(error) => {
            log::error!("JSON serialization error: {error}");
            send_internal_error(req, format_args!("JSON serialization error: {error}"))?;
        }
    }
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

fn send_json_rsp<C: Connection, T: Serialize + fmt::Display>(
    req: Request<C>,
    json: JsonResponse<T>,
) -> HandlerResult {
    log::info!(
        "responding with {} {}: {}",
        json.code,
        json.status,
        json.message
    );
    let mut rsp = req.into_response(
        json.code,
        Some(json.status),
        &[(header::CONTENT_TYPE, content_type::JSON)],
    )?;
    // TODO(eliza): don't allocate here...
    let json = serde_json::to_string_pretty(&json)?;
    rsp.write_all(json.as_bytes())?;
    Ok(())
}

fn send_bad_request<C: Connection>(req: Request<C>, error: impl fmt::Display) -> HandlerResult {
    // TODO(eliza): don't ToString these...
    send_json_rsp(
        req,
        JsonResponse {
            code: 400,
            status: "Bad Request",
            message: error.to_string(),
        },
    )
}

fn send_internal_error<C: Connection>(req: Request<C>, error: impl fmt::Display) -> HandlerResult {
    // TODO(eliza): don't ToString these...
    send_json_rsp(
        req,
        JsonResponse {
            code: 500,
            status: "Internal Server Error",
            message: error.to_string(),
        },
    )
}

#[derive(serde::Serialize)]
struct JsonResponse<T: Serialize> {
    code: u16,
    status: &'static str,
    message: T,
}

mod header {
    pub(super) const CONTENT_TYPE: &str = "content-type";
}

mod content_type {

    pub(super) const JSON: &str = "application/json";
    pub(super) const HTML: &str = "text/html";
}
