use crate::{wifi, SensorMetrics};
use anyhow::Context;
use embedded_svc::{
    http::Method,
    io::{Read, Write},
};
use esp_idf_svc::http::server::EspHttpServer;
use std::sync::mpsc;

pub struct Server {
    pub wifi_credentials: mpsc::Receiver<wifi::Credentials>,
    server: EspHttpServer,
}

pub fn start_server(
    access_points: wifi::AccessPoints,
    metrics: &'static SensorMetrics,
) -> anyhow::Result<Server> {
    let mut server =
        EspHttpServer::new(&Default::default()).context("failed to start HTTP server")?;
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
            let content = r#"
                <!DOCTYPE html>
                <html>
                    <body>
                        Submitted!
                    </body>
                </html>
                "#;
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
                &[("content-type", "text/plain; version=0.0.4")],
            )?;
            metrics.render_prometheus(&mut rsp)?;

            Ok(())
        })
        .context("adding GET /metrics handler")?;

    log::info!("Server is running on http://192.168.71.1/");

    Ok(Server {
        wifi_credentials: rx,
        server,
    })
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

    log::trace!("truncated {} bytes", buf.len() - total_bytes_read);
    buf.truncate(total_bytes_read);

    Ok(total_bytes_read)
}
