use crate::{actor, net, scd30, SensorMetrics};
use anyhow::{anyhow, Context};
use embedded_svc::{
    http::{
        server::{Connection, HandlerResult, Request},
        Method,
    },
    io::{Read, Write},
};
use esp_idf_svc::http::server::{Configuration, EspHttpConnection, EspHttpServer};
use futures::{channel::mpsc, SinkExt, StreamExt};
use serde::Serialize;
use std::future::Future;

pub struct Server {
    _server: EspHttpServer,
    wifi_client: actor::Client<net::Credentials, anyhow::Result<()>>,
    buffer: Vec<u8>,
}

pub const HTTP_PORT: u16 = 80;
pub const HTTPS_PORT: u16 = 443;

enum BgReq<C> {
    WifiSelect(Request<C>),
}

pub fn start_server(
    wifi: &net::EclssWifi,
    metrics: &'static SensorMetrics,
    // scd30_client: actor::Client<scd30::ControlMessage, anyhow::Result<()>>,
) -> anyhow::Result<impl Future<Output = anyhow::Result<()>> + 'static> {
    let mut server = EspHttpServer::new(&Configuration {
        http_port: HTTP_PORT,
        https_port: HTTPS_PORT,
        ..Default::default()
    })
    .context("failed to start HTTP server")?;

    let (bg_tx, bg_rx) = mpsc::channel::<BgReq<&mut EspHttpConnection<'_>>>(16);

    let access_points = wifi.access_points.clone();
    let wifi_client = wifi.credentials_tx();
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
        .fn_handler("/wifi/select", Method::Post, {
            let tx = bg_tx.clone();
            move |mut req| {
                tx.try_send(BgReq::WifiSelect(req));
                Ok(())
            }
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
            serve_json(req, metrics)
        })
        .context("adding GET /sensors.json handler")?
        .fn_handler("/wifi/ssids.json", Method::Get, move |req| {
            let ssids = access_points.read().unwrap();
            let ssids = ssids.iter().map(|ap| &ap.ssid).collect::<Vec<_>>();
            serve_json(req, &ssids)
        })
        .context("adding GET /wifi/ssids.json handler")?;

    log::info!("Server is running on http://192.168.71.1/");

    let server = Server {
        _server: server,
        wifi_client,
        buffer: vec![0; 40],
    };
    Ok(server.run(bg_rx))
}

impl Server {
    async fn run<C: Connection>(
        mut self,
        mut reqs: mpsc::Receiver<BgReq<C>>,
    ) -> anyhow::Result<()> {
        log::info!("started HTTP background task...");
        loop {
            self.buffer.clear();
            match reqs.next().await {
                None => {
                    log::warn!("HTTP server channel closed!");
                    anyhow::bail!("HTTP server channel closed!");
                }
                Some(BgReq::WifiSelect(req)) => match self.wifi_select(req).await {
                    Ok(()) => log::info!("handled /wifi/select route"),
                    Err(error) => log::warn!("/wifi/select failed: {error}"),
                },
            };
        }
    }
    async fn wifi_select<C: Connection>(&mut self, mut req: Request<C>) -> anyhow::Result<()> {
        read_body(&mut req, &mut self.buffer)?;

        let credentials = serde_urlencoded::from_bytes(&self.buffer)?;
        match self.wifi_client.try_request(credentials).await {
            Ok(_) => {
                let content = r#"<!DOCTYPE html><html><body>Submitted!</body></html>"#;
                req.into_response(
                    200,
                    Some("OK"),
                    &[(header::CONTENT_TYPE, content_type::HTML)],
                )
                .map_err(|error| anyhow!("error writing response: {error:?}"))?
                .write_all(content.as_bytes())
                .map_err(|error| anyhow!("error writing response: {error:?}"))
            }
            Err(error) => {
                let content = format!("<!DOCTYPE html><html><body>{error:?}</body></html>");
                req.into_response(
                    500,
                    Some("Internal Server Error"),
                    &[(header::CONTENT_TYPE, content_type::HTML)],
                )
                .map_err(|error| anyhow!("error writing response: {error:?}"))?
                .write_all(content.as_bytes())
                .map_err(|error| anyhow!("error writing response: {error:?}"))
            }
        }
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
