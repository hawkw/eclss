use crate::{net, SensorMetrics};
use anyhow::Context;
use embedded_svc::{
    http::{
        server::{Connection, HandlerResult, Request},
        Headers, Method,
    },
    io::{Read, Write},
};
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use std::sync::mpsc;

pub struct Server {
    pub wifi_credentials: mpsc::Receiver<net::Credentials>,
    server: EspHttpServer,
}

pub const HTTP_PORT: u16 = 80;
pub const HTTPS_PORT: u16 = 443;

pub fn start_server(
    access_points: net::AccessPoints,
    metrics: &'static SensorMetrics,
) -> anyhow::Result<Server> {
    let mut server = EspHttpServer::new(&Configuration {
        http_port: HTTP_PORT,
        https_port: HTTPS_PORT,
        ..Default::default()
    })
    .context("failed to start HTTP server")?;
    let (tx, rx) = mpsc::sync_channel(1);
    server
        .fn_handler("/", Method::Get, move |req| {
            let mut ssids = String::new();
            for ap in access_points.read().unwrap().iter() {
                use std::fmt::Write;
                write!(&mut ssids, "<option>{ssid}</option>", ssid = ap.ssid)?;
            }
            let rsp = format!(include_str!("./http/index.html"), ssids);
            req.into_ok_response()?.write_all(rsp.as_bytes())?;

            Ok(())
        })
        .context("adding GET / handler")?
        .fn_handler("/wifi-select", Method::Post, move |mut req| {
            let mut body = vec![0; 40];
            read_body(&mut req, &mut body)?;

            let credentials = serde_urlencoded::from_bytes(&body)?;
            let content = r#"<!DOCTYPE html><html><body>Submitted!</body></html>"#;
            req.into_ok_response()?.write_all(content.as_bytes())?;
            tx.send(credentials).context("sending wifi credentials")?;
            Ok(())
        })
        .context("adding POST /wifi-select handler")?
        // TODO(eliza): also serve this on the normal prometheus metrics port?
        .fn_handler("/metrics", Method::Get, move |req| {
            let mut rsp = req.into_response(
                200,
                Some("OK"),
                &[(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            )?;
            metrics.render_prometheus(&mut rsp)?;

            Ok(())
        })
        .context("adding GET /metrics handler")?
        .fn_handler("/sensors.json", Method::Get, move |req| {
            get_sensors(req, metrics)
        })
        .context("adding GET /sensors.json handler")?;

    log::info!("Server is running on http://192.168.71.1/");

    Ok(Server {
        wifi_credentials: rx,
        server,
    })
}

fn get_sensors<C: Connection>(req: Request<C>, sensors: &'static SensorMetrics) -> HandlerResult {
    const JSON: &str = "application/json";

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

    let mut rsp = req.into_response(200, Some("OK"), &[(header::CONTENT_TYPE, JSON)])?;
    // TODO(eliza): don't allocate here...
    let json = serde_json::to_string_pretty(&sensors)?;
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

mod header {
    pub(super) const CONTENT_TYPE: &str = "content-type";
}
