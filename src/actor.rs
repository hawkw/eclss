//! A request-response actor channel.
use futures::channel::oneshot;
use thingbuf::{mpsc, Recycle};

pub fn channel<Req, Rsp>(capacity: usize) -> (Client<Req, Rsp>, Actor<Req, Rsp>) {
    let (tx, rx) = mpsc::with_recycle(capacity, ChanRecycle);
    (Client(tx), Actor(rx))
}

pub struct Client<Req, Rsp>(mpsc::Sender<Slot<Req, Rsp>, ChanRecycle>);

pub struct Actor<Req, Rsp>(mpsc::Receiver<Slot<Req, Rsp>, ChanRecycle>);

pub struct Envelope<Req, Rsp> {
    req: Req,
    rsp_tx: oneshot::Sender<Rsp>,
}

pub enum ReqError<Req> {
    Closed(Req),
    RspCanceled,
}

pub enum TryReqError<Req, Err> {
    Error(Err),
    Closed(Req),
    RspCanceled,
}

type Slot<Req, Rsp> = Option<Envelope<Req, Rsp>>;

struct ChanRecycle;

impl<Req, Rsp> Client<Req, Rsp> {
    pub async fn send_request(&self, req: Req) -> Result<Rsp, ReqError<Req>> {
        let (rsp_tx, rsp_rx) = oneshot::channel();
        self.0
            .send(Some(Envelope { req, rsp_tx }))
            .await
            .map_err(|closed| {
                let req = closed.into_inner().unwrap().req;
                ReqError::Closed(req)
            })?;
        rsp_rx.await.map_err(|_| ReqError::RspCanceled)
    }

    pub fn try_send(&self, req: Req) -> Result<oneshot::Receiver<Rsp>, ReqError<Req>> {
        let (rsp_tx, rsp_rx) = oneshot::channel();
        self.0
            .try_send(Some(Envelope { req, rsp_tx }))
            .map_err(|closed| {
                let req = closed.into_inner().unwrap().req;
                ReqError::Closed(req)
            })
            .map(|_| rsp_rx)
    }
}

impl<Req, Rsp, Err> Client<Req, Result<Rsp, Err>> {
    pub async fn try_request(&self, req: Req) -> Result<Rsp, TryReqError<Req, Err>> {
        match self.send_request(req).await {
            Ok(Ok(rsp)) => Ok(rsp),
            Ok(Err(error)) => Err(TryReqError::Error(error)),
            Err(ReqError::Closed(req)) => Err(TryReqError::Closed(req)),
            Err(ReqError::RspCanceled) => Err(TryReqError::RspCanceled),
        }
    }
}

impl<Req, Rsp> Clone for Client<Req, Rsp> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// === impl Server ===

impl<Req, Rsp> Actor<Req, Rsp> {
    pub async fn next_request(&mut self) -> Option<Envelope<Req, Rsp>> {
        let req = self.0.recv().await?;
        debug_assert!(
            req.is_some(),
            "empty envelope should never be received! this is a bug!"
        );
        req
    }
}

// === impl Envelope ===

impl<Req, Rsp> Envelope<Req, Rsp> {
    pub fn request(&self) -> &Req {
        &self.req
    }

    pub fn respond(self, rsp: Rsp) -> Result<(), Rsp> {
        self.rsp_tx.send(rsp)
    }
}

// === impl ChanRecycle ===

impl<Req, Rsp> Recycle<Slot<Req, Rsp>> for ChanRecycle {
    fn new_element(&self) -> Slot<Req, Rsp> {
        None
    }

    fn recycle(&self, element: &mut Slot<Req, Rsp>) {
        *element = None;
    }
}
