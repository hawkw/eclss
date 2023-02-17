use crate::{net, SensorMetrics};
use anyhow::Context;
use embedded_svc::{
    http::{
        server::{Connection, HandlerResult, Request},
        Method,
    },
    io::{Read, Write},
    ws::asynch::server::Acceptor as WsAcceptor,
};
use esp_idf_svc::http::server::{ws::EspHttpWsProcessor, Configuration, EspHttpServer};
use serde::Serialize;
use std::sync::Mutex;

pub struct Server {
    _server: EspHttpServer,
}

pub const HTTP_PORT: u16 = 80;
pub const HTTPS_PORT: u16 = 443;
const MAX_WS_CONNS: usize = 16;
const MAX_WS_FRAME_LEN: usize = 512;

pub fn start_server(
    wifi: &net::EclssWifi,
    metrics: &'static SensorMetrics,
) -> anyhow::Result<(Server, impl WsAcceptor)> {
    let (ws_processor, ws_acceptor) = EspHttpWsProcessor::<MAX_WS_CONNS, MAX_WS_FRAME_LEN>::new(());

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
            req.into_response(
                200,
                Some("OK"),
                &[(header::CONTENT_TYPE, content_type::HTML)],
            )?
            .write_all(INDEX)?;
            Ok(())
        })
        .context("adding GET / handler")?
        .fn_handler("/wifi/select", Method::Post, move |mut req| {
            let mut body = vec![0; 40];
            read_body(&mut req, &mut body)?;

            let credentials = serde_urlencoded::from_bytes(&body)?;
            let content = r#"<!DOCTYPE html><html><body>Submitted!</body></html>"#;
            req.into_response(
                200,
                Some("OK"),
                &[(header::CONTENT_TYPE, content_type::HTML)],
            )?
            .write_all(content.as_bytes())?;
            creds_tx
                .try_send(credentials)
                .context("sending wifi credentials")?;
            Ok(())
        })
        .context("adding POST /wifi/select handler")?
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
            log::info!("handling request for sensors.json");
            serve_json(req, metrics)
        })
        .context("adding GET /sensors.json handler")?
        .fn_handler("/wifi/ssids.json", Method::Get, move |req| {
            let ssids = access_points.read().unwrap();
            let ssids = ssids.iter().map(|ap| &ap.ssid).collect::<Vec<_>>();
            serve_json(req, &ssids)
        })
        .context("adding GET /wifi/ssids.json handler")?
        .ws_handler("/ws", {
            let ws = Mutex::new(ws_processor);
            move |conn| ws.lock().unwrap().process(conn)
        })
        .context("adding websocket handler")?;

    log::info!("Server is running on http://192.168.71.1/");

    Ok((Server { _server: server }, ws_acceptor))
}

pub async fn serve_ws(ws: impl WsAcceptor, metrics: &'static SensorMetrics) -> anyhow::Result<()> {
    loop {
        let (tx, rx) = match ws.accept().await {
            Ok(x) => x,
            Err(error) => {
                log::error!("failed to accept websocket connection: {error:?}");
                continue;
            }
        };
        log::info!("accepted ws conn");
    }
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

    let mut rsp = req.into_response(
        200,
        Some("OK"),
        &[(header::CONTENT_TYPE, content_type::JSON)],
    )?;
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

mod header {
    pub(super) const CONTENT_TYPE: &str = "content-type";
}

mod content_type {

    pub(super) const JSON: &str = "application/json";
    pub(super) const HTML: &str = "text/html";
}
